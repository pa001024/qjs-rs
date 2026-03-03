#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

use ast::{
    BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier, Module,
    ModuleExport, ModuleImport, ModuleImportBinding, ObjectProperty, ObjectPropertyKey, Script,
    Stmt, StringLiteral, SwitchCase, UnaryOp, UpdateTarget, VariableDeclaration,
};
use lexer::{Token, TokenKind, lex};

const NON_SIMPLE_PARAMS_MARKER: &str = "$__qjs_non_simple_params__$";
const ARROW_FUNCTION_MARKER: &str = "$__qjs_arrow_function__$";
const PARAM_INIT_SCOPE_START_MARKER: &str = "$__qjs_param_init_scope_start__$";
const PARAM_INIT_SCOPE_END_MARKER: &str = "$__qjs_param_init_scope_end__$";
const REST_PARAM_MARKER_PREFIX: &str = "$__qjs_rest_param__$";
const CLASS_CONSTRUCTOR_MARKER: &str = "$__qjs_class_constructor__$";
const CLASS_DERIVED_CONSTRUCTOR_MARKER: &str = "$__qjs_class_derived_constructor__$";
const CLASS_CONSTRUCTOR_PARENT_MARKER: &str = "$__qjs_class_constructor_parent__$";
const CLASS_HERITAGE_RESTRICTED_MARKER: &str = "$__qjs_class_heritage_restricted__$";
const CLASS_METHOD_NO_PROTOTYPE_MARKER: &str = "$__qjs_class_method_no_prototype__$";
const GENERATOR_FUNCTION_MARKER: &str = "$__qjs_generator_function__$";
const ASYNC_FUNCTION_MARKER: &str = "$__qjs_async_function__$";
const NAMED_FUNCTION_EXPR_MARKER: &str = "$__qjs_named_function_expr__$";
const CLASS_CONSTRUCTOR_SUPER_BASE_BINDING: &str = "$__qjs_super_base__$";
static NEXT_TAGGED_TEMPLATE_SITE_ID: AtomicU64 = AtomicU64::new(1);

type DefaultParameterInitializer = (Identifier, Expr);
type ParsedParameterList = (
    Vec<Identifier>,
    bool,
    Vec<DefaultParameterInitializer>,
    Vec<Stmt>,
);
type ParsedTemplateLiteralParts = (Vec<StringLiteral>, Vec<StringLiteral>, Vec<bool>, Vec<Expr>);

fn next_tagged_template_site_id() -> u64 {
    NEXT_TAGGED_TEMPLATE_SITE_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

pub fn parse_expression(source: &str) -> Result<Expr, ParseError> {
    let tokens = lex(source).map_err(|err| ParseError {
        message: err.message,
        position: err.position,
    })?;
    let mut parser = Parser::new(tokens, source);
    let expr = parser.parse_expression_with_commas()?;
    validate_expression_strict_mode(&expr, false)?;
    parser.expect_eof()?;
    Ok(expr)
}

pub fn parse_script(source: &str) -> Result<Script, ParseError> {
    parse_script_with_super(source, false)
}

pub fn parse_script_with_super(
    source: &str,
    allow_super_reference: bool,
) -> Result<Script, ParseError> {
    let tokens = lex(source).map_err(|err| ParseError {
        message: err.message,
        position: err.position,
    })?;
    let mut parser = Parser::new(tokens, source);
    parser.allow_super_reference = allow_super_reference;
    let statements = parser.parse_statement_list(None)?;
    validate_early_errors(&statements)?;
    validate_statement_list_strict_mode(&statements, false)?;
    parser.expect_eof()?;
    Ok(Script { statements })
}

pub fn parse_module(source: &str) -> Result<Module, ParseError> {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut exported_names = BTreeSet::new();
    let mut transformed_lines = Vec::new();
    let mut default_export_index = 0usize;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            transformed_lines.push(raw_line.to_string());
            continue;
        }
        if trimmed.starts_with("import ") {
            imports.push(parse_module_import_declaration(trimmed)?);
            continue;
        }
        if trimmed.starts_with("export ") {
            let export_body = trimmed
                .strip_prefix("export ")
                .expect("prefix checked")
                .trim();
            if export_body.starts_with('{') {
                for export in parse_named_export_clause(export_body)? {
                    register_module_export(&mut exports, &mut exported_names, export)?;
                }
                continue;
            }
            if export_body.starts_with('*') {
                return Err(ParseError {
                    message: "unsupported export form".to_string(),
                    position: 0,
                });
            }
            if let Some(default_expr) = export_body.strip_prefix("default ") {
                let default_expr = default_expr.trim();
                if !default_expr.ends_with(';') {
                    return Err(ParseError {
                        message: "module declaration must end with ';'".to_string(),
                        position: 0,
                    });
                }
                let expr = default_expr[..default_expr.len() - 1].trim();
                if expr.is_empty() {
                    return Err(ParseError {
                        message: "export default requires an expression".to_string(),
                        position: 0,
                    });
                }
                let local = format!("$__qjs_module_default_export_{default_export_index}__$");
                default_export_index += 1;
                transformed_lines.push(format!("const {local} = {expr};"));
                register_module_export(
                    &mut exports,
                    &mut exported_names,
                    ModuleExport {
                        exported: "default".to_string(),
                        local,
                    },
                )?;
                continue;
            }

            if export_body.contains(" from ") {
                return Err(ParseError {
                    message: "unsupported export re-export form".to_string(),
                    position: 0,
                });
            }
            for local in collect_module_declared_bindings(export_body)? {
                register_module_export(
                    &mut exports,
                    &mut exported_names,
                    ModuleExport {
                        exported: local.clone(),
                        local,
                    },
                )?;
            }
            transformed_lines.push(export_body.to_string());
            continue;
        }

        transformed_lines.push(raw_line.to_string());
    }

    if !exports.is_empty() {
        transformed_lines.push(render_module_export_snapshot_statement(&exports));
    }
    let transformed_source = transformed_lines.join("\n");
    let body = parse_script_with_super(&transformed_source, false)?;
    Ok(Module {
        imports,
        exports,
        body,
    })
}

fn parse_module_import_declaration(line: &str) -> Result<ModuleImport, ParseError> {
    let mut rest = line.strip_prefix("import ").expect("prefix checked").trim();
    if !rest.ends_with(';') {
        return Err(ParseError {
            message: "module declaration must end with ';'".to_string(),
            position: 0,
        });
    }
    rest = rest[..rest.len() - 1].trim();

    if rest.starts_with('\'') || rest.starts_with('"') {
        return Ok(ModuleImport {
            specifier: parse_module_string_literal(rest)?,
            bindings: Vec::new(),
        });
    }

    let Some((raw_clause, raw_specifier)) = rest.rsplit_once(" from ") else {
        return Err(ParseError {
            message: "unsupported import declaration form".to_string(),
            position: 0,
        });
    };
    let clause = raw_clause.trim();
    let specifier = parse_module_string_literal(raw_specifier.trim())?;
    let bindings = parse_module_import_clause_bindings(clause)?;
    Ok(ModuleImport {
        specifier,
        bindings,
    })
}

fn parse_module_import_clause_bindings(
    clause: &str,
) -> Result<Vec<ModuleImportBinding>, ParseError> {
    if clause.starts_with('{') {
        return parse_named_import_bindings(clause);
    }
    if let Some(local) = clause.strip_prefix("* as ") {
        return Ok(vec![ModuleImportBinding {
            imported: "*".to_string(),
            local: parse_module_binding_identifier(local.trim())?,
        }]);
    }

    let mut bindings = Vec::new();
    if let Some((default_binding, remainder)) = clause.split_once(',') {
        bindings.push(ModuleImportBinding {
            imported: "default".to_string(),
            local: parse_module_binding_identifier(default_binding.trim())?,
        });
        let remainder = remainder.trim();
        if remainder.starts_with('{') {
            bindings.extend(parse_named_import_bindings(remainder)?);
        } else if let Some(local) = remainder.strip_prefix("* as ") {
            bindings.push(ModuleImportBinding {
                imported: "*".to_string(),
                local: parse_module_binding_identifier(local.trim())?,
            });
        } else {
            return Err(ParseError {
                message: "unsupported import declaration form".to_string(),
                position: 0,
            });
        }
        return Ok(bindings);
    }

    bindings.push(ModuleImportBinding {
        imported: "default".to_string(),
        local: parse_module_binding_identifier(clause.trim())?,
    });
    Ok(bindings)
}

fn parse_named_import_bindings(clause: &str) -> Result<Vec<ModuleImportBinding>, ParseError> {
    let inner = parse_braced_clause_inner(clause)?;
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let mut bindings = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (imported, local) = if let Some((imported, local)) = part.split_once(" as ") {
            (
                parse_module_import_name(imported.trim())?,
                parse_module_binding_identifier(local.trim())?,
            )
        } else {
            let ident = parse_module_binding_identifier(part)?;
            (ident.clone(), ident)
        };
        bindings.push(ModuleImportBinding { imported, local });
    }
    Ok(bindings)
}

fn parse_named_export_clause(clause: &str) -> Result<Vec<ModuleExport>, ParseError> {
    if !clause.ends_with(';') {
        return Err(ParseError {
            message: "module declaration must end with ';'".to_string(),
            position: 0,
        });
    }
    let body = clause[..clause.len() - 1].trim();
    let inner = parse_braced_clause_inner(body)?;
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let mut exports = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let export = if let Some((local, exported)) = part.split_once(" as ") {
            ModuleExport {
                local: parse_module_binding_identifier(local.trim())?,
                exported: parse_module_export_name(exported.trim())?,
            }
        } else {
            let local = parse_module_binding_identifier(part)?;
            ModuleExport {
                local: local.clone(),
                exported: local,
            }
        };
        exports.push(export);
    }
    Ok(exports)
}

fn collect_module_declared_bindings(declaration: &str) -> Result<Vec<String>, ParseError> {
    let declaration = declaration.trim();
    if let Some(rest) = declaration.strip_prefix("const ") {
        return parse_variable_export_bindings(rest);
    }
    if let Some(rest) = declaration.strip_prefix("let ") {
        return parse_variable_export_bindings(rest);
    }
    if let Some(rest) = declaration.strip_prefix("var ") {
        return parse_variable_export_bindings(rest);
    }
    if let Some(rest) = declaration.strip_prefix("function ") {
        let name = parse_leading_identifier(rest)?;
        return Ok(vec![name]);
    }
    if let Some(rest) = declaration.strip_prefix("async function ") {
        let name = parse_leading_identifier(rest)?;
        return Ok(vec![name]);
    }
    if let Some(rest) = declaration.strip_prefix("class ") {
        let name = parse_leading_identifier(rest)?;
        return Ok(vec![name]);
    }
    Err(ParseError {
        message: "unsupported export declaration form".to_string(),
        position: 0,
    })
}

fn parse_variable_export_bindings(declarators: &str) -> Result<Vec<String>, ParseError> {
    if !declarators.trim_end().ends_with(';') {
        return Err(ParseError {
            message: "module declaration must end with ';'".to_string(),
            position: 0,
        });
    }
    let body = declarators.trim_end();
    let body = body[..body.len() - 1].trim();
    if body.is_empty() {
        return Err(ParseError {
            message: "unsupported export declaration form".to_string(),
            position: 0,
        });
    }

    let mut names = Vec::new();
    for declarator in body.split(',') {
        let declarator = declarator.trim();
        if declarator.is_empty() {
            continue;
        }
        let lhs = declarator
            .split_once('=')
            .map_or(declarator, |(left, _)| left)
            .trim();
        if lhs.contains('{') || lhs.contains('[') {
            return Err(ParseError {
                message: "unsupported destructuring export declaration".to_string(),
                position: 0,
            });
        }
        names.push(parse_module_binding_identifier(lhs)?);
    }
    if names.is_empty() {
        return Err(ParseError {
            message: "unsupported export declaration form".to_string(),
            position: 0,
        });
    }
    Ok(names)
}

fn parse_leading_identifier(source: &str) -> Result<String, ParseError> {
    let mut chars = source.trim_start().chars().peekable();
    let mut ident = String::new();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            ident.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    parse_module_binding_identifier(&ident)
}

fn parse_braced_clause_inner(clause: &str) -> Result<String, ParseError> {
    let clause = clause.trim();
    if !clause.starts_with('{') || !clause.ends_with('}') {
        return Err(ParseError {
            message: "unsupported module declaration form".to_string(),
            position: 0,
        });
    }
    Ok(clause[1..clause.len() - 1].trim().to_string())
}

fn parse_module_string_literal(source: &str) -> Result<String, ParseError> {
    let source = source.trim();
    let quote = source.chars().next().ok_or(ParseError {
        message: "expected module specifier".to_string(),
        position: 0,
    })?;
    if quote != '\'' && quote != '"' {
        return Err(ParseError {
            message: "expected module specifier string literal".to_string(),
            position: 0,
        });
    }
    if !source.ends_with(quote) || source.len() < 2 {
        return Err(ParseError {
            message: "unterminated module specifier literal".to_string(),
            position: 0,
        });
    }
    Ok(source[1..source.len() - 1].to_string())
}

fn parse_module_import_name(name: &str) -> Result<String, ParseError> {
    if name == "default" {
        return Ok(name.to_string());
    }
    parse_module_binding_identifier(name)
}

fn parse_module_export_name(name: &str) -> Result<String, ParseError> {
    if name == "default" {
        return Ok(name.to_string());
    }
    parse_module_binding_identifier(name)
}

fn parse_module_binding_identifier(name: &str) -> Result<String, ParseError> {
    let name = name.trim();
    if !is_valid_module_identifier(name) || is_forbidden_binding_identifier(name) {
        return Err(ParseError {
            message: "invalid module binding identifier".to_string(),
            position: 0,
        });
    }
    Ok(name.to_string())
}

fn is_valid_module_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn register_module_export(
    exports: &mut Vec<ModuleExport>,
    exported_names: &mut BTreeSet<String>,
    export: ModuleExport,
) -> Result<(), ParseError> {
    if !exported_names.insert(export.exported.clone()) {
        return Err(ParseError {
            message: "duplicate export binding".to_string(),
            position: 0,
        });
    }
    exports.push(export);
    Ok(())
}

fn render_module_export_snapshot_statement(exports: &[ModuleExport]) -> String {
    let mut rendered = String::from("({");
    for (index, export) in exports.iter().enumerate() {
        if index > 0 {
            rendered.push_str(", ");
        }
        rendered.push_str(&export.exported);
        rendered.push_str(": ");
        rendered.push_str(&export.local);
    }
    rendered.push_str("});");
    rendered
}

fn validate_early_errors(statements: &[Stmt]) -> Result<(), ParseError> {
    validate_label_control_targets(statements)?;
    validate_statement_list_early_errors(statements, StatementListKind::ScriptOrFunction, false)
}

fn validate_statement_list_strict_mode(
    statements: &[Stmt],
    inherited_strict: bool,
) -> Result<(), ParseError> {
    let strict = inherited_strict || statement_list_has_use_strict_directive(statements);
    for statement in statements {
        validate_statement_strict_mode(statement, strict)?;
    }
    Ok(())
}

fn statement_list_has_use_strict_directive(statements: &[Stmt]) -> bool {
    for statement in statements {
        match statement {
            Stmt::Expression(Expr::String(StringLiteral { value, has_escape })) => {
                if value == "use strict" && !has_escape {
                    return true;
                }
            }
            Stmt::Empty => break,
            _ => break,
        }
    }
    false
}

fn validate_statement_strict_mode(statement: &Stmt, strict: bool) -> Result<(), ParseError> {
    match statement {
        Stmt::Empty => Ok(()),
        Stmt::VariableDeclaration(VariableDeclaration {
            name: Identifier(name),
            initializer,
            ..
        }) => {
            validate_binding_identifier_strict_mode(name, strict)?;
            if let Some(initializer) = initializer {
                validate_expression_strict_mode(initializer, strict)?;
            }
            Ok(())
        }
        Stmt::VariableDeclarations(declarations) => {
            for declaration in declarations {
                validate_binding_identifier_strict_mode(&declaration.name.0, strict)?;
                if let Some(initializer) = &declaration.initializer {
                    validate_expression_strict_mode(initializer, strict)?;
                }
            }
            Ok(())
        }
        Stmt::FunctionDeclaration(FunctionDeclaration { name, params, body }) => {
            let function_strict = strict || statement_list_has_use_strict_directive(body);
            validate_binding_identifier_strict_mode(&name.0, function_strict)?;
            validate_unique_parameter_names_strict_mode(params, function_strict)?;
            for param in params {
                validate_binding_identifier_strict_mode(&param.0, function_strict)?;
            }
            validate_statement_list_strict_mode(body, function_strict)
        }
        Stmt::Return(value) => {
            if let Some(value) = value {
                validate_expression_strict_mode(value, strict)?;
            }
            Ok(())
        }
        Stmt::Expression(expr) | Stmt::Throw(expr) => validate_expression_strict_mode(expr, strict),
        Stmt::Block(statements) => validate_statement_list_strict_mode(statements, strict),
        Stmt::If {
            condition,
            consequent,
            alternate,
        } => {
            validate_expression_strict_mode(condition, strict)?;
            validate_statement_strict_mode(consequent, strict)?;
            if let Some(alternate) = alternate {
                validate_statement_strict_mode(alternate, strict)?;
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            validate_expression_strict_mode(condition, strict)?;
            validate_statement_strict_mode(body, strict)
        }
        Stmt::With { object, body } => {
            if strict {
                return Err(ParseError {
                    message: "with statement not allowed in strict mode".to_string(),
                    position: 0,
                });
            }
            validate_expression_strict_mode(object, strict)?;
            validate_statement_strict_mode(body, strict)
        }
        Stmt::DoWhile { body, condition } => {
            validate_statement_strict_mode(body, strict)?;
            validate_expression_strict_mode(condition, strict)
        }
        Stmt::For {
            initializer,
            condition,
            update,
            body,
        } => {
            if let Some(initializer) = initializer {
                match initializer {
                    ForInitializer::VariableDeclaration(declaration) => {
                        validate_binding_identifier_strict_mode(&declaration.name.0, strict)?;
                        if let Some(initializer) = &declaration.initializer {
                            validate_expression_strict_mode(initializer, strict)?;
                        }
                    }
                    ForInitializer::VariableDeclarations(declarations) => {
                        for declaration in declarations {
                            validate_binding_identifier_strict_mode(&declaration.name.0, strict)?;
                            if let Some(initializer) = &declaration.initializer {
                                validate_expression_strict_mode(initializer, strict)?;
                            }
                        }
                    }
                    ForInitializer::Expression(expr) => {
                        validate_expression_strict_mode(expr, strict)?;
                    }
                }
            }
            if let Some(condition) = condition {
                validate_expression_strict_mode(condition, strict)?;
            }
            if let Some(update) = update {
                validate_expression_strict_mode(update, strict)?;
            }
            validate_statement_strict_mode(body, strict)
        }
        Stmt::Switch {
            discriminant,
            cases,
        } => {
            validate_expression_strict_mode(discriminant, strict)?;
            for case in cases {
                if let Some(test) = &case.test {
                    validate_expression_strict_mode(test, strict)?;
                }
                validate_statement_list_strict_mode(&case.consequent, strict)?;
            }
            Ok(())
        }
        Stmt::Try {
            try_block,
            catch_param,
            catch_block,
            finally_block,
        } => {
            validate_statement_list_strict_mode(try_block, strict)?;
            if let Some(catch_param) = catch_param {
                validate_binding_identifier_strict_mode(&catch_param.0, strict)?;
            }
            if let Some(catch_block) = catch_block {
                validate_statement_list_strict_mode(catch_block, strict)?;
            }
            if let Some(finally_block) = finally_block {
                validate_statement_list_strict_mode(finally_block, strict)?;
            }
            Ok(())
        }
        Stmt::Labeled { body, .. } => validate_statement_strict_mode(body, strict),
        Stmt::Break | Stmt::BreakLabel(_) | Stmt::Continue | Stmt::ContinueLabel(_) => Ok(()),
    }
}

fn validate_expression_strict_mode(expr: &Expr, strict: bool) -> Result<(), ParseError> {
    match expr {
        Expr::Number(_)
        | Expr::Bool(_)
        | Expr::Null
        | Expr::String(_)
        | Expr::RegexLiteral { .. }
        | Expr::Elision => Ok(()),
        Expr::This => Ok(()),
        Expr::Identifier(Identifier(name)) => {
            validate_identifier_reference_strict_mode(name, strict)
        }
        Expr::Function { name, params, body } => {
            let function_strict = strict || statement_list_has_use_strict_directive(body);
            if let Some(name) = name {
                validate_binding_identifier_strict_mode(&name.0, function_strict)?;
            }
            validate_unique_parameter_names_strict_mode(params, function_strict)?;
            for param in params {
                validate_binding_identifier_strict_mode(&param.0, function_strict)?;
            }
            validate_statement_list_strict_mode(body, function_strict)
        }
        Expr::ObjectLiteral(properties) => {
            for property in properties {
                match &property.key {
                    ObjectPropertyKey::Computed(key)
                    | ObjectPropertyKey::AccessorGetComputed(key)
                    | ObjectPropertyKey::AccessorSetComputed(key) => {
                        validate_expression_strict_mode(key, strict)?;
                    }
                    ObjectPropertyKey::Static(_)
                    | ObjectPropertyKey::ProtoSetter
                    | ObjectPropertyKey::AccessorGet(_)
                    | ObjectPropertyKey::AccessorSet(_) => {}
                }
                validate_expression_strict_mode(&property.value, strict)?;
            }
            Ok(())
        }
        Expr::ArrayLiteral(elements) => {
            for element in elements {
                validate_expression_strict_mode(element, strict)?;
            }
            Ok(())
        }
        Expr::Unary { expr, .. } => validate_expression_strict_mode(expr, strict),
        Expr::Conditional {
            condition,
            consequent,
            alternate,
        } => {
            validate_expression_strict_mode(condition, strict)?;
            validate_expression_strict_mode(consequent, strict)?;
            validate_expression_strict_mode(alternate, strict)
        }
        Expr::Sequence(expressions) => {
            for expression in expressions {
                validate_expression_strict_mode(expression, strict)?;
            }
            Ok(())
        }
        Expr::Member { object, .. } => validate_expression_strict_mode(object, strict),
        Expr::MemberComputed { object, property } => {
            validate_expression_strict_mode(object, strict)?;
            validate_expression_strict_mode(property, strict)
        }
        Expr::Call { callee, arguments } | Expr::New { callee, arguments } => {
            validate_expression_strict_mode(callee, strict)?;
            for argument in arguments {
                validate_expression_strict_mode(argument, strict)?;
            }
            Ok(())
        }
        Expr::Binary { left, right, .. } => {
            validate_expression_strict_mode(left, strict)?;
            validate_expression_strict_mode(right, strict)
        }
        Expr::Assign {
            target: Identifier(name),
            value,
        } => {
            validate_identifier_reference_strict_mode(name, strict)?;
            validate_assignment_target_identifier_strict_mode(name, strict)?;
            validate_expression_strict_mode(value, strict)
        }
        Expr::AssignMember { object, value, .. } => {
            validate_expression_strict_mode(object, strict)?;
            validate_expression_strict_mode(value, strict)
        }
        Expr::AssignMemberComputed {
            object,
            property,
            value,
        } => {
            validate_expression_strict_mode(object, strict)?;
            validate_expression_strict_mode(property, strict)?;
            validate_expression_strict_mode(value, strict)
        }
        Expr::Update { target, .. } => match target {
            UpdateTarget::Identifier(Identifier(name)) => {
                validate_identifier_reference_strict_mode(name, strict)?;
                validate_assignment_target_identifier_strict_mode(name, strict)
            }
            UpdateTarget::Member { object, .. } => validate_expression_strict_mode(object, strict),
            UpdateTarget::MemberComputed { object, property } => {
                validate_expression_strict_mode(object, strict)?;
                validate_expression_strict_mode(property, strict)
            }
        },
        Expr::AnnexBCallAssignmentTarget { target } => {
            if strict {
                return Err(ParseError {
                    message: "invalid assignment target".to_string(),
                    position: 0,
                });
            }
            validate_expression_strict_mode(target, strict)
        }
        Expr::SpreadArgument(expr) => validate_expression_strict_mode(expr, strict),
    }
}

fn validate_binding_identifier_strict_mode(name: &str, strict: bool) -> Result<(), ParseError> {
    if strict && is_strict_mode_restricted_binding_name(name) {
        return Err(ParseError {
            message: "invalid binding identifier in strict mode".to_string(),
            position: 0,
        });
    }
    if strict && is_strict_mode_future_reserved_word(name) {
        return Err(ParseError {
            message: "reserved word cannot be binding identifier".to_string(),
            position: 0,
        });
    }
    Ok(())
}

fn validate_identifier_reference_strict_mode(name: &str, strict: bool) -> Result<(), ParseError> {
    if strict && is_strict_mode_future_reserved_word(name) {
        return Err(ParseError {
            message: "reserved word cannot be identifier reference".to_string(),
            position: 0,
        });
    }
    Ok(())
}

fn validate_assignment_target_identifier_strict_mode(
    name: &str,
    strict: bool,
) -> Result<(), ParseError> {
    if strict && is_strict_mode_restricted_binding_name(name) {
        return Err(ParseError {
            message: "invalid lvalue in strict mode".to_string(),
            position: 0,
        });
    }
    Ok(())
}

fn validate_unique_parameter_names_strict_mode(
    params: &[Identifier],
    strict: bool,
) -> Result<(), ParseError> {
    if !strict {
        return Ok(());
    }
    let mut seen = BTreeSet::new();
    for param in params {
        let name = param.0.as_str();
        if !seen.insert(name) {
            return Err(ParseError {
                message: "duplicate parameter name in strict mode".to_string(),
                position: 0,
            });
        }
    }
    Ok(())
}

fn is_strict_mode_restricted_binding_name(name: &str) -> bool {
    matches!(name, "eval" | "arguments")
}

fn is_strict_mode_future_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "implements" | "interface" | "package" | "private" | "protected" | "public" | "static"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LabelTarget {
    name: String,
    continue_is_valid: bool,
}

fn validate_label_control_targets(statements: &[Stmt]) -> Result<(), ParseError> {
    let mut label_targets = Vec::new();
    validate_label_control_targets_in_statements(statements, &mut label_targets)
}

fn validate_label_control_targets_in_statements(
    statements: &[Stmt],
    label_targets: &mut Vec<LabelTarget>,
) -> Result<(), ParseError> {
    for statement in statements {
        validate_label_control_targets_in_statement(statement, label_targets)?;
    }
    Ok(())
}

fn validate_label_control_targets_in_statement(
    statement: &Stmt,
    label_targets: &mut Vec<LabelTarget>,
) -> Result<(), ParseError> {
    match statement {
        Stmt::Labeled { label, body } => {
            label_targets.push(LabelTarget {
                name: label.0.clone(),
                continue_is_valid: statement_allows_continue_label(body),
            });
            let result = validate_label_control_targets_in_statement(body, label_targets);
            label_targets.pop();
            result
        }
        Stmt::BreakLabel(Identifier(label)) => {
            if !label_targets
                .iter()
                .rev()
                .any(|target| target.name == *label)
            {
                return Err(ParseError {
                    message: format!("undefined label: {label}"),
                    position: 0,
                });
            }
            Ok(())
        }
        Stmt::ContinueLabel(Identifier(label)) => {
            let Some(target) = label_targets
                .iter()
                .rev()
                .find(|target| target.name == *label)
            else {
                return Err(ParseError {
                    message: format!("undefined label: {label}"),
                    position: 0,
                });
            };
            if !target.continue_is_valid {
                return Err(ParseError {
                    message: "continue target must be iteration statement".to_string(),
                    position: 0,
                });
            }
            Ok(())
        }
        Stmt::Block(statements) => {
            validate_label_control_targets_in_statements(statements, label_targets)
        }
        Stmt::FunctionDeclaration(declaration) => validate_label_control_targets(&declaration.body),
        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            validate_label_control_targets_in_statement(consequent, label_targets)?;
            if let Some(alternate) = alternate {
                validate_label_control_targets_in_statement(alternate, label_targets)?;
            }
            Ok(())
        }
        Stmt::While { body, .. }
        | Stmt::With { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. } => {
            validate_label_control_targets_in_statement(body, label_targets)
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                validate_label_control_targets_in_statements(&case.consequent, label_targets)?;
            }
            Ok(())
        }
        Stmt::Try {
            try_block,
            catch_block,
            finally_block,
            ..
        } => {
            validate_label_control_targets_in_statements(try_block, label_targets)?;
            if let Some(catch_block) = catch_block {
                validate_label_control_targets_in_statements(catch_block, label_targets)?;
            }
            if let Some(finally_block) = finally_block {
                validate_label_control_targets_in_statements(finally_block, label_targets)?;
            }
            Ok(())
        }
        Stmt::Empty
        | Stmt::VariableDeclaration(_)
        | Stmt::VariableDeclarations(_)
        | Stmt::Return(_)
        | Stmt::Expression(_)
        | Stmt::Throw(_)
        | Stmt::Break
        | Stmt::Continue => Ok(()),
    }
}

fn statement_allows_continue_label(statement: &Stmt) -> bool {
    match statement {
        Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::For { .. } => true,
        Stmt::Labeled { body, .. } => statement_allows_continue_label(body),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementListKind {
    ScriptOrFunction,
    BlockLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexicalDeclarationKind {
    FunctionDeclaration,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
enum ClassMethodKey {
    Static(String),
    Computed(Expr),
}

#[derive(Debug, Clone, PartialEq)]
enum ClassElementKind {
    Method,
    Getter,
    Setter,
}

#[derive(Debug, Clone, PartialEq)]
struct ClassMethodDefinition {
    key: ClassMethodKey,
    value: Expr,
    is_static: bool,
    kind: ClassElementKind,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct ParsedClassTail {
    methods: Vec<ClassMethodDefinition>,
    extends: Option<Expr>,
}

fn validate_statement_list_early_errors(
    statements: &[Stmt],
    kind: StatementListKind,
    inherited_strict: bool,
) -> Result<(), ParseError> {
    let strict = inherited_strict
        || (kind == StatementListKind::ScriptOrFunction
            && statement_list_has_use_strict_directive(statements));

    let mut lexical_names = BTreeMap::new();
    for statement in statements {
        collect_direct_lexical_names(statement, &mut lexical_names, kind, strict)?;
    }

    let mut var_declared_names = BTreeSet::new();
    for statement in statements {
        collect_var_declared_names(statement, &mut var_declared_names, kind);
    }

    if let Some(name) = lexical_names
        .keys()
        .find(|candidate| var_declared_names.contains(*candidate))
    {
        return Err(ParseError {
            message: format!("lexical declaration conflicts with var/function declaration: {name}"),
            position: 0,
        });
    }

    for statement in statements {
        validate_nested_statement_early_errors(statement, strict)?;
    }

    Ok(())
}

fn validate_nested_statement_early_errors(
    statement: &Stmt,
    strict: bool,
) -> Result<(), ParseError> {
    match statement {
        Stmt::Block(statements) => {
            validate_statement_list_early_errors(statements, StatementListKind::BlockLike, strict)
        }
        Stmt::FunctionDeclaration(declaration) => validate_statement_list_early_errors(
            &declaration.body,
            StatementListKind::ScriptOrFunction,
            strict,
        ),
        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            validate_nested_statement_early_errors(consequent, strict)?;
            if let Some(alternate) = alternate {
                validate_nested_statement_early_errors(alternate, strict)?;
            }
            Ok(())
        }
        Stmt::While { body, .. }
        | Stmt::With { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::Labeled { body, .. } => validate_nested_statement_early_errors(body, strict),
        Stmt::Switch { cases, .. } => validate_switch_case_early_errors(cases, strict),
        Stmt::Try {
            try_block,
            catch_param,
            catch_block,
            finally_block,
        } => {
            validate_statement_list_early_errors(try_block, StatementListKind::BlockLike, strict)?;
            if let Some(catch_block) = catch_block {
                validate_catch_block_early_errors(catch_param.as_ref(), catch_block, strict)?;
            }
            if let Some(finally_block) = finally_block {
                validate_statement_list_early_errors(
                    finally_block,
                    StatementListKind::BlockLike,
                    strict,
                )?;
            }
            Ok(())
        }
        Stmt::Empty
        | Stmt::VariableDeclaration(_)
        | Stmt::VariableDeclarations(_)
        | Stmt::Return(_)
        | Stmt::Expression(_)
        | Stmt::Throw(_)
        | Stmt::Break
        | Stmt::BreakLabel(_)
        | Stmt::Continue
        | Stmt::ContinueLabel(_) => Ok(()),
    }
}

fn validate_switch_case_early_errors(cases: &[SwitchCase], strict: bool) -> Result<(), ParseError> {
    let mut lexical_names = BTreeMap::new();
    for case in cases {
        for statement in &case.consequent {
            collect_direct_lexical_names(
                statement,
                &mut lexical_names,
                StatementListKind::BlockLike,
                strict,
            )?;
        }
    }

    let mut var_declared_names = BTreeSet::new();
    for case in cases {
        for statement in &case.consequent {
            collect_var_declared_names(
                statement,
                &mut var_declared_names,
                StatementListKind::BlockLike,
            );
        }
    }

    if let Some(name) = lexical_names
        .keys()
        .find(|candidate| var_declared_names.contains(*candidate))
    {
        return Err(ParseError {
            message: format!("lexical declaration conflicts with var/function declaration: {name}"),
            position: 0,
        });
    }

    for case in cases {
        for statement in &case.consequent {
            validate_nested_statement_early_errors(statement, strict)?;
        }
    }

    Ok(())
}

fn validate_catch_block_early_errors(
    catch_param: Option<&Identifier>,
    catch_block: &[Stmt],
    strict: bool,
) -> Result<(), ParseError> {
    validate_statement_list_early_errors(catch_block, StatementListKind::BlockLike, strict)?;

    if let Some(catch_param) = catch_param {
        let mut lexical_names = BTreeMap::new();
        for statement in catch_block {
            collect_direct_lexical_names(
                statement,
                &mut lexical_names,
                StatementListKind::BlockLike,
                strict,
            )?;
        }
        if lexical_names.contains_key(&catch_param.0) {
            return Err(ParseError {
                message: format!(
                    "catch parameter conflicts with lexical declaration: {}",
                    catch_param.0
                ),
                position: 0,
            });
        }
    }

    Ok(())
}

fn collect_direct_lexical_names(
    statement: &Stmt,
    lexical_names: &mut BTreeMap<String, LexicalDeclarationKind>,
    kind: StatementListKind,
    strict: bool,
) -> Result<(), ParseError> {
    match statement {
        Stmt::VariableDeclaration(declaration) => {
            add_lexical_name_if_needed(lexical_names, declaration, strict)?;
        }
        Stmt::VariableDeclarations(declarations) => {
            for declaration in declarations {
                add_lexical_name_if_needed(lexical_names, declaration, strict)?;
            }
        }
        Stmt::FunctionDeclaration(declaration) => {
            if kind == StatementListKind::BlockLike {
                add_lexical_name(
                    lexical_names,
                    &declaration.name.0,
                    LexicalDeclarationKind::FunctionDeclaration,
                    strict,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn add_lexical_name_if_needed(
    lexical_names: &mut BTreeMap<String, LexicalDeclarationKind>,
    declaration: &VariableDeclaration,
    strict: bool,
) -> Result<(), ParseError> {
    if !matches!(declaration.kind, BindingKind::Let | BindingKind::Const) {
        return Ok(());
    }
    add_lexical_name(
        lexical_names,
        &declaration.name.0,
        LexicalDeclarationKind::Other,
        strict,
    )
}

fn add_lexical_name(
    lexical_names: &mut BTreeMap<String, LexicalDeclarationKind>,
    name: &str,
    kind: LexicalDeclarationKind,
    strict: bool,
) -> Result<(), ParseError> {
    if let Some(existing_kind) = lexical_names.get(name) {
        if !strict
            && *existing_kind == LexicalDeclarationKind::FunctionDeclaration
            && kind == LexicalDeclarationKind::FunctionDeclaration
        {
            return Ok(());
        }
        return Err(ParseError {
            message: format!("duplicate lexical declaration: {name}"),
            position: 0,
        });
    }
    lexical_names.insert(name.to_string(), kind);
    Ok(())
}

fn collect_var_declared_names(
    statement: &Stmt,
    var_declared_names: &mut BTreeSet<String>,
    kind: StatementListKind,
) {
    match statement {
        Stmt::VariableDeclaration(declaration) => {
            add_var_name_if_needed(var_declared_names, declaration);
        }
        Stmt::VariableDeclarations(declarations) => {
            for declaration in declarations {
                add_var_name_if_needed(var_declared_names, declaration);
            }
        }
        Stmt::FunctionDeclaration(declaration) => {
            if kind == StatementListKind::ScriptOrFunction {
                var_declared_names.insert(declaration.name.0.clone());
            }
        }
        Stmt::Block(statements) => {
            for statement in statements {
                collect_var_declared_names(
                    statement,
                    var_declared_names,
                    StatementListKind::BlockLike,
                );
            }
        }
        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            collect_var_declared_names(
                consequent,
                var_declared_names,
                StatementListKind::BlockLike,
            );
            if let Some(alternate) = alternate {
                collect_var_declared_names(
                    alternate,
                    var_declared_names,
                    StatementListKind::BlockLike,
                );
            }
        }
        Stmt::While { body, .. }
        | Stmt::With { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::Labeled { body, .. } => {
            collect_var_declared_names(body, var_declared_names, StatementListKind::BlockLike);
        }
        Stmt::For {
            initializer, body, ..
        } => {
            if let Some(initializer) = initializer {
                collect_var_declared_names_from_for_initializer(initializer, var_declared_names);
            }
            collect_var_declared_names(body, var_declared_names, StatementListKind::BlockLike);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for statement in &case.consequent {
                    collect_var_declared_names(
                        statement,
                        var_declared_names,
                        StatementListKind::BlockLike,
                    );
                }
            }
        }
        Stmt::Try {
            try_block,
            catch_block,
            finally_block,
            ..
        } => {
            for statement in try_block {
                collect_var_declared_names(
                    statement,
                    var_declared_names,
                    StatementListKind::BlockLike,
                );
            }
            if let Some(catch_block) = catch_block {
                for statement in catch_block {
                    collect_var_declared_names(
                        statement,
                        var_declared_names,
                        StatementListKind::BlockLike,
                    );
                }
            }
            if let Some(finally_block) = finally_block {
                for statement in finally_block {
                    collect_var_declared_names(
                        statement,
                        var_declared_names,
                        StatementListKind::BlockLike,
                    );
                }
            }
        }
        Stmt::Empty
        | Stmt::Return(_)
        | Stmt::Expression(_)
        | Stmt::Throw(_)
        | Stmt::Break
        | Stmt::BreakLabel(_)
        | Stmt::Continue
        | Stmt::ContinueLabel(_) => {}
    }
}

fn collect_var_declared_names_from_for_initializer(
    initializer: &ForInitializer,
    var_declared_names: &mut BTreeSet<String>,
) {
    match initializer {
        ForInitializer::VariableDeclaration(declaration) => {
            add_var_name_if_needed(var_declared_names, declaration);
        }
        ForInitializer::VariableDeclarations(declarations) => {
            for declaration in declarations {
                add_var_name_if_needed(var_declared_names, declaration);
            }
        }
        ForInitializer::Expression(_) => {}
    }
}

fn add_var_name_if_needed(
    var_declared_names: &mut BTreeSet<String>,
    declaration: &VariableDeclaration,
) {
    if declaration.kind == BindingKind::Var {
        var_declared_names.insert(declaration.name.0.clone());
    }
}

fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "let"
    )
}

fn is_forbidden_identifier_reference(name: &str) -> bool {
    if matches!(name, "yield" | "await" | "let") {
        return false;
    }
    is_reserved_word(name)
}

fn is_forbidden_binding_identifier(name: &str) -> bool {
    if matches!(name, "yield" | "await") {
        return false;
    }
    is_reserved_word(name)
}

#[derive(Debug, Clone)]
struct ForHeadArrayPatternElement {
    index: usize,
    name: Identifier,
    default_initializer: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignmentKind {
    Simple,
    Compound(BinaryOp),
    LogicalAnd,
    LogicalOr,
    Nullish,
}

#[derive(Debug)]
struct Parser {
    tokens: Vec<Token>,
    source: String,
    pos: usize,
    expression_depth: usize,
    allow_in: bool,
    function_depth: usize,
    loop_depth: usize,
    breakable_depth: usize,
    label_stack: Vec<String>,
    class_temp_index: usize,
    allow_super_reference: bool,
    async_function_depth: usize,
    generator_function_depth: usize,
    generator_yield_bindings: Vec<String>,
    generator_yield_seen: Vec<bool>,
}

impl Parser {
    fn new(tokens: Vec<Token>, source: &str) -> Self {
        Self {
            tokens,
            source: source.to_string(),
            pos: 0,
            expression_depth: 0,
            allow_in: true,
            function_depth: 0,
            loop_depth: 0,
            breakable_depth: 0,
            label_stack: Vec::new(),
            class_temp_index: 0,
            allow_super_reference: false,
            async_function_depth: 0,
            generator_function_depth: 0,
            generator_yield_bindings: Vec::new(),
            generator_yield_seen: Vec::new(),
        }
    }

    fn parse_statement_list(
        &mut self,
        terminator: Option<TokenKind>,
    ) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();

        loop {
            if let Some(term) = &terminator {
                if self.check(term) {
                    break;
                }
            }
            if self.is_eof() {
                break;
            }
            if self.matches(&TokenKind::Semicolon) {
                statements.push(Stmt::Empty);
                continue;
            }

            let starts_class_declaration = self.check_keyword("class");
            let statement = self.parse_statement()?;
            let needs_separator = !matches!(
                statement,
                Stmt::Block(_)
                    | Stmt::Empty
                    | Stmt::FunctionDeclaration(_)
                    | Stmt::If { .. }
                    | Stmt::While { .. }
                    | Stmt::With { .. }
                    | Stmt::DoWhile { .. }
                    | Stmt::For { .. }
                    | Stmt::Switch { .. }
                    | Stmt::Labeled { .. }
                    | Stmt::Try { .. }
            ) && !starts_class_declaration;
            statements.push(statement);

            if self.matches(&TokenKind::Semicolon) {
                continue;
            }
            if let Some(term) = &terminator {
                if self.check(term) {
                    continue;
                }
            }
            if self.is_eof() {
                break;
            }
            if self.has_line_terminator_between_prev_and_current() {
                continue;
            }
            if needs_separator {
                return Err(self.error_current("expected ';' between statements"));
            }
        }

        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.matches(&TokenKind::Semicolon) {
            return Ok(Stmt::Empty);
        }
        if self.check(&TokenKind::LBrace) {
            return self.parse_block_statement();
        }
        if self.check_keyword("async")
            && self.check_next_keyword("function")
            && !self.has_line_terminator_between_tokens(self.pos, self.pos + 1)
        {
            self.advance();
            self.advance();
            return self.parse_function_declaration_statement(true);
        }
        if self.matches_keyword("function") {
            return self.parse_function_declaration_statement(false);
        }
        if self.matches_keyword("class") {
            return self.parse_class_declaration_statement();
        }
        if self.matches_keyword("if") {
            return self.parse_if_statement();
        }
        if self.matches_keyword("while") {
            return self.parse_while_statement();
        }
        if self.matches_keyword("do") {
            return self.parse_do_while_statement();
        }
        if self.matches_keyword("for") {
            return self.parse_for_statement();
        }
        if self.matches_keyword("switch") {
            return self.parse_switch_statement();
        }
        if self.matches_keyword("with") {
            return self.parse_with_statement();
        }
        if self.matches_keyword("try") {
            return self.parse_try_statement();
        }
        if self.matches_keyword("debugger") {
            return Ok(Stmt::Empty);
        }
        if self.matches_keyword("throw") {
            return self.parse_throw_statement();
        }
        if self.matches_keyword("break") {
            return self.parse_break_statement();
        }
        if self.matches_keyword("continue") {
            return self.parse_continue_statement();
        }
        if self.matches_keyword("return") {
            return self.parse_return_statement();
        }
        if self.generator_function_depth > 0 && self.matches_keyword("yield") {
            return self.parse_generator_yield_statement();
        }
        if self.check_keyword("let") && self.statement_starts_with_lexical_let_declaration() {
            self.advance();
            return self.parse_variable_declaration(BindingKind::Let);
        }
        if self.matches_keyword("const") {
            return self.parse_variable_declaration(BindingKind::Const);
        }
        if self.matches_keyword("var") {
            return self.parse_variable_declaration(BindingKind::Var);
        }
        if self.check_identifier() && self.check_next(&TokenKind::Colon) {
            return self.parse_labeled_statement();
        }
        let expr = self.parse_expression_with_commas()?;
        Ok(Stmt::Expression(expr))
    }

    fn parse_block_statement(&mut self) -> Result<Stmt, ParseError> {
        let statements =
            self.parse_block_body("expected '{' to start block", "expected '}' after block")?;
        Ok(Stmt::Block(statements))
    }

    fn parse_block_body(
        &mut self,
        start_error: &str,
        end_error: &str,
    ) -> Result<Vec<Stmt>, ParseError> {
        self.expect(TokenKind::LBrace, start_error)?;
        let statements = self.parse_statement_list(Some(TokenKind::RBrace))?;
        self.expect(TokenKind::RBrace, end_error)?;
        Ok(statements)
    }

    fn parse_function_body(
        &mut self,
        start_error: &str,
        end_error: &str,
    ) -> Result<Vec<Stmt>, ParseError> {
        self.function_depth += 1;
        let saved_label_stack = std::mem::take(&mut self.label_stack);
        let body = self.parse_block_body(start_error, end_error);
        self.label_stack = saved_label_stack;
        self.function_depth = self.function_depth.saturating_sub(1);
        body
    }

    fn parse_function_body_with_super_policy(
        &mut self,
        start_error: &str,
        end_error: &str,
        allow_super_reference: bool,
        is_generator: bool,
    ) -> Result<Vec<Stmt>, ParseError> {
        self.parse_function_body_with_context(
            start_error,
            end_error,
            allow_super_reference,
            false,
            is_generator,
        )
    }

    fn parse_function_body_with_context(
        &mut self,
        start_error: &str,
        end_error: &str,
        allow_super_reference: bool,
        is_async: bool,
        is_generator: bool,
    ) -> Result<Vec<Stmt>, ParseError> {
        let saved_allow_super_reference = self.allow_super_reference;
        let saved_async_function_depth = self.async_function_depth;
        let saved_generator_function_depth = self.generator_function_depth;
        self.allow_super_reference = allow_super_reference;
        self.async_function_depth = if is_async {
            saved_async_function_depth + 1
        } else {
            0
        };
        self.generator_function_depth = if is_generator {
            saved_generator_function_depth + 1
        } else {
            0
        };
        let generator_binding = if is_generator {
            let binding = format!("$__qjs_generator_values_{}__$", self.class_temp_index);
            self.class_temp_index += 1;
            self.generator_yield_bindings.push(binding.clone());
            self.generator_yield_seen.push(false);
            Some(binding)
        } else {
            None
        };
        let mut body = self.parse_function_body(start_error, end_error);
        if let Some(binding) = generator_binding {
            let saw_yield = self.generator_yield_seen.pop().unwrap_or(false);
            let _ = self.generator_yield_bindings.pop();
            if saw_yield && let Ok(statements) = body.as_mut() {
                self.lower_generator_yield_statements(statements, &binding);
            }
        }
        self.allow_super_reference = saved_allow_super_reference;
        self.async_function_depth = saved_async_function_depth;
        self.generator_function_depth = saved_generator_function_depth;
        body
    }

    fn parse_generator_yield_statement(&mut self) -> Result<Stmt, ParseError> {
        let binding_name = self
            .generator_yield_bindings
            .last()
            .cloned()
            .ok_or_else(|| ParseError {
                message: "yield is only valid in generator function".to_string(),
                position: self.previous_position(),
            })?;
        let delegated = self.matches(&TokenKind::Star);
        let omit_expression = !delegated
            && (self.check(&TokenKind::Semicolon)
                || self.check(&TokenKind::RBrace)
                || self.is_eof()
                || self.has_line_terminator_between_prev_and_current());
        let yielded = if delegated {
            self.parse_expression_with_commas()?
        } else if omit_expression {
            Expr::Identifier(Identifier("undefined".to_string()))
        } else {
            self.parse_expression_with_commas()?
        };
        if let Some(seen) = self.generator_yield_seen.last_mut() {
            *seen = true;
        }
        Ok(Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(Expr::Identifier(Identifier(binding_name))),
                property: "push".to_string(),
            }),
            arguments: vec![yielded],
        }))
    }

    fn lower_generator_yield_statements(&self, body: &mut Vec<Stmt>, binding_name: &str) {
        let declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: Identifier(binding_name.to_string()),
            initializer: Some(Expr::ArrayLiteral(Vec::new())),
        });
        let mut insert_at = 0usize;
        while insert_at < body.len() {
            match &body[insert_at] {
                Stmt::Expression(Expr::String(StringLiteral { has_escape, .. })) if !has_escape => {
                    insert_at += 1;
                }
                _ => break,
            }
        }
        body.insert(insert_at, declaration);
        body.push(Stmt::Return(Some(Expr::Identifier(Identifier(
            binding_name.to_string(),
        )))));
    }

    fn prepend_marker(&self, body: &mut Vec<Stmt>, marker: &str) {
        body.insert(
            0,
            Stmt::Expression(Expr::String(StringLiteral {
                value: marker.to_string(),
                has_escape: false,
            })),
        );
    }

    fn insert_no_prototype_marker(&self, body: &mut Vec<Stmt>) {
        let marker = Stmt::Expression(Expr::String(StringLiteral {
            value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
            has_escape: false,
        }));
        let mut insert_at = 0usize;
        while insert_at < body.len() {
            match &body[insert_at] {
                Stmt::Expression(Expr::String(StringLiteral { has_escape, .. })) if !has_escape => {
                    insert_at += 1;
                }
                _ => break,
            }
        }
        body.insert(insert_at, marker);
    }

    fn insert_generator_function_marker(&self, body: &mut Vec<Stmt>) {
        let marker = Stmt::Expression(Expr::String(StringLiteral {
            value: GENERATOR_FUNCTION_MARKER.to_string(),
            has_escape: false,
        }));
        let mut insert_at = 0usize;
        while insert_at < body.len() {
            match &body[insert_at] {
                Stmt::Expression(Expr::String(StringLiteral { has_escape, .. })) if !has_escape => {
                    insert_at += 1;
                }
                _ => break,
            }
        }
        body.insert(insert_at, marker);
    }

    fn insert_async_function_marker(&self, body: &mut Vec<Stmt>) {
        let marker = Stmt::Expression(Expr::String(StringLiteral {
            value: ASYNC_FUNCTION_MARKER.to_string(),
            has_escape: false,
        }));
        let mut insert_at = 0usize;
        while insert_at < body.len() {
            match &body[insert_at] {
                Stmt::Expression(Expr::String(StringLiteral { has_escape, .. })) if !has_escape => {
                    insert_at += 1;
                }
                _ => break,
            }
        }
        body.insert(insert_at, marker);
    }

    fn insert_named_function_expression_marker(&self, body: &mut Vec<Stmt>) {
        let marker = Stmt::Expression(Expr::String(StringLiteral {
            value: NAMED_FUNCTION_EXPR_MARKER.to_string(),
            has_escape: false,
        }));
        let mut insert_at = 0usize;
        while insert_at < body.len() {
            match &body[insert_at] {
                Stmt::Expression(Expr::String(StringLiteral { has_escape, .. })) if !has_escape => {
                    insert_at += 1;
                }
                _ => break,
            }
        }
        body.insert(insert_at, marker);
    }

    fn prepend_non_simple_params_marker(&self, body: &mut Vec<Stmt>) {
        self.prepend_marker(body, NON_SIMPLE_PARAMS_MARKER);
    }

    fn prepend_arrow_function_marker(&self, body: &mut Vec<Stmt>) {
        self.prepend_marker(body, ARROW_FUNCTION_MARKER);
    }

    fn prepend_parameter_initializers(
        &self,
        body: &mut Vec<Stmt>,
        initializers: &[DefaultParameterInitializer],
        pattern_effects: &[Stmt],
    ) {
        if initializers.is_empty() && pattern_effects.is_empty() {
            return;
        }
        let mut prefix = Vec::with_capacity(initializers.len() + pattern_effects.len() + 2);
        prefix.push(Stmt::Expression(Expr::String(StringLiteral {
            value: PARAM_INIT_SCOPE_START_MARKER.to_string(),
            has_escape: false,
        })));
        for (param, default_value) in initializers {
            let condition = Expr::Binary {
                op: BinaryOp::StrictEqual,
                left: Box::new(Expr::Identifier(param.clone())),
                right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
            };
            let assignment = Stmt::Expression(Expr::Assign {
                target: param.clone(),
                value: Box::new(default_value.clone()),
            });
            prefix.push(Stmt::If {
                condition,
                consequent: Box::new(assignment),
                alternate: None,
            });
        }
        prefix.extend(pattern_effects.iter().cloned());
        prefix.push(Stmt::Expression(Expr::String(StringLiteral {
            value: PARAM_INIT_SCOPE_END_MARKER.to_string(),
            has_escape: false,
        })));
        prefix.append(body);
        *body = prefix;
    }

    fn parse_function_declaration_statement(&mut self, is_async: bool) -> Result<Stmt, ParseError> {
        let is_generator = self.matches(&TokenKind::Star);
        let name = Identifier(self.expect_binding_identifier("expected function name")?);
        self.expect(TokenKind::LParen, "expected '(' after function name")?;
        let (params, simple_parameters, default_initializers, pattern_effects) =
            self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        let mut body = self.parse_function_body_with_context(
            "expected '{' before function body",
            "expected '}' after function body",
            false,
            is_async,
            is_generator,
        )?;
        self.prepend_parameter_initializers(&mut body, &default_initializers, &pattern_effects);
        if !simple_parameters {
            self.prepend_non_simple_params_marker(&mut body);
        }
        if is_async {
            self.insert_async_function_marker(&mut body);
        }
        if is_generator {
            self.insert_generator_function_marker(&mut body);
        }

        Ok(Stmt::FunctionDeclaration(FunctionDeclaration {
            name,
            params,
            body,
        }))
    }

    fn parse_class_declaration_statement(&mut self) -> Result<Stmt, ParseError> {
        let name = Identifier(self.expect_binding_identifier("expected class name")?);
        let class_tail = self.parse_class_tail()?;
        let initializer = self.lower_class_tail(class_tail, Some(name.clone()));
        Ok(Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name,
            initializer: Some(initializer),
        }))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'if'")?;
        let condition = self.parse_expression_with_commas()?;
        self.expect(TokenKind::RParen, "expected ')' after if condition")?;

        let consequent = self.parse_embedded_statement(true)?;
        let alternate = if self.matches_keyword("else") {
            Some(Box::new(self.parse_embedded_statement(false)?))
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            consequent: Box::new(consequent),
            alternate,
        })
    }

    fn parse_while_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.parse_expression_with_commas()?;
        self.expect(TokenKind::RParen, "expected ')' after while condition")?;
        self.loop_depth += 1;
        self.breakable_depth += 1;
        let body = self.parse_embedded_statement(false);
        self.loop_depth = self.loop_depth.saturating_sub(1);
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
        let body = body?;
        Ok(Stmt::While {
            condition,
            body: Box::new(body),
        })
    }

    fn parse_do_while_statement(&mut self) -> Result<Stmt, ParseError> {
        self.loop_depth += 1;
        self.breakable_depth += 1;
        let body = self.parse_embedded_statement(false);
        self.loop_depth = self.loop_depth.saturating_sub(1);
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
        let body = body?;

        if !self.matches_keyword("while") {
            return Err(self.error_current("expected 'while' after do-while body"));
        }
        self.expect(TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.parse_expression_with_commas()?;
        self.expect(TokenKind::RParen, "expected ')' after do-while condition")?;
        let _ = self.matches(&TokenKind::Semicolon);
        Ok(Stmt::DoWhile {
            body: Box::new(body),
            condition,
        })
    }

    fn parse_for_statement(&mut self) -> Result<Stmt, ParseError> {
        let is_await = self.matches_keyword("await");
        if is_await && self.async_function_depth == 0 {
            return Err(self.error_current("for-await is only valid in async functions"));
        }
        self.expect(TokenKind::LParen, "expected '(' after 'for'")?;

        let initializer = if self.check(&TokenKind::Semicolon) {
            None
        } else if self.check_keyword("let") && self.for_head_starts_with_lexical_let_declaration() {
            self.matches_keyword("let");
            if self.matches(&TokenKind::LBracket) {
                let elements =
                    self.parse_for_head_array_pattern_after_lbracket(BindingKind::Let)?;
                if self.check_keyword("in") || self.check_keyword("of") {
                    return self
                        .parse_for_in_of_array_pattern_statement(BindingKind::Let, elements);
                }
                let declarations =
                    self.parse_for_head_array_pattern_declaration(BindingKind::Let, elements)?;
                Some(ForInitializer::VariableDeclarations(declarations))
            } else if self.check(&TokenKind::LBrace) {
                let binding_name = self.next_for_in_temp_identifier("for_object");
                let effects = self.parse_object_parameter_pattern_effects(&binding_name)?;
                if self.check_keyword("in") || self.check_keyword("of") {
                    return self.parse_for_in_of_object_pattern_statement(
                        BindingKind::Let,
                        binding_name,
                        effects,
                    );
                }
                let declarations = self.parse_for_head_object_pattern_declaration(
                    BindingKind::Let,
                    binding_name,
                    effects,
                )?;
                Some(ForInitializer::VariableDeclarations(declarations))
            } else {
                Some(self.parse_for_head_variable_declaration(BindingKind::Let)?)
            }
        } else if self.matches_keyword("const") {
            if self.matches(&TokenKind::LBracket) {
                let elements =
                    self.parse_for_head_array_pattern_after_lbracket(BindingKind::Const)?;
                if self.check_keyword("in") || self.check_keyword("of") {
                    return self
                        .parse_for_in_of_array_pattern_statement(BindingKind::Const, elements);
                }
                let declarations =
                    self.parse_for_head_array_pattern_declaration(BindingKind::Const, elements)?;
                Some(ForInitializer::VariableDeclarations(declarations))
            } else if self.check(&TokenKind::LBrace) {
                let binding_name = self.next_for_in_temp_identifier("for_object");
                let effects = self.parse_object_parameter_pattern_effects(&binding_name)?;
                if self.check_keyword("in") || self.check_keyword("of") {
                    return self.parse_for_in_of_object_pattern_statement(
                        BindingKind::Const,
                        binding_name,
                        effects,
                    );
                }
                let declarations = self.parse_for_head_object_pattern_declaration(
                    BindingKind::Const,
                    binding_name,
                    effects,
                )?;
                Some(ForInitializer::VariableDeclarations(declarations))
            } else {
                Some(self.parse_for_head_variable_declaration(BindingKind::Const)?)
            }
        } else if self.matches_keyword("var") {
            if self.matches(&TokenKind::LBracket) {
                let elements =
                    self.parse_for_head_array_pattern_after_lbracket(BindingKind::Var)?;
                if self.check_keyword("in") || self.check_keyword("of") {
                    return self
                        .parse_for_in_of_array_pattern_statement(BindingKind::Var, elements);
                }
                let declarations =
                    self.parse_for_head_array_pattern_declaration(BindingKind::Var, elements)?;
                Some(ForInitializer::VariableDeclarations(declarations))
            } else if self.check(&TokenKind::LBrace) {
                let binding_name = self.next_for_in_temp_identifier("for_object");
                let effects = self.parse_object_parameter_pattern_effects(&binding_name)?;
                if self.check_keyword("in") || self.check_keyword("of") {
                    return self.parse_for_in_of_object_pattern_statement(
                        BindingKind::Var,
                        binding_name,
                        effects,
                    );
                }
                let declarations = self.parse_for_head_object_pattern_declaration(
                    BindingKind::Var,
                    binding_name,
                    effects,
                )?;
                Some(ForInitializer::VariableDeclarations(declarations))
            } else {
                Some(self.parse_for_head_variable_declaration(BindingKind::Var)?)
            }
        } else {
            Some(ForInitializer::Expression(self.parse_expression_no_in()?))
        };
        let loop_kind = if self.matches_keyword("in") {
            Some("in")
        } else if self.matches_keyword("of") {
            Some("of")
        } else {
            None
        };
        if is_await && loop_kind != Some("of") {
            return Err(self.error_current("for-await only supports 'of'"));
        }
        if let Some(loop_kind) = loop_kind {
            let iterable = self.parse_for_in_of_rhs_expression()?;
            self.expect(TokenKind::RParen, "expected ')' after for-in/of clauses")?;

            self.loop_depth += 1;
            self.breakable_depth += 1;
            let body = self.parse_embedded_statement(false);
            self.loop_depth = self.loop_depth.saturating_sub(1);
            self.breakable_depth = self.breakable_depth.saturating_sub(1);
            let body = body?;

            if is_await {
                if Self::supports_for_of_lowering(initializer.as_ref()) {
                    return self.lower_for_await_of_statement(initializer, iterable, body);
                }
                return Ok(Stmt::For {
                    initializer: None,
                    condition: Some(Expr::Bool(false)),
                    update: None,
                    body: Box::new(body),
                });
            }
            if loop_kind == "in" && Self::supports_for_in_lowering(initializer.as_ref()) {
                return self.lower_for_in_statement(initializer, iterable, body);
            }
            if loop_kind == "of" && Self::supports_for_of_lowering(initializer.as_ref()) {
                return self.lower_for_of_statement(initializer, iterable, body);
            }
            // Baseline: parse/compile unsupported `for-in`/`for-of` shapes as non-iterating loops.
            return Ok(Stmt::For {
                initializer: None,
                condition: Some(Expr::Bool(false)),
                update: None,
                body: Box::new(body),
            });
        }
        self.expect(TokenKind::Semicolon, "expected ';' after for initializer")?;

        let condition = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression_with_commas()?)
        };
        self.expect(TokenKind::Semicolon, "expected ';' after for condition")?;

        let update = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(self.parse_expression_with_commas()?)
        };
        self.expect(TokenKind::RParen, "expected ')' after for clauses")?;

        self.loop_depth += 1;
        self.breakable_depth += 1;
        let body = self.parse_embedded_statement(false);
        self.loop_depth = self.loop_depth.saturating_sub(1);
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
        let body = body?;

        Ok(Stmt::For {
            initializer,
            condition,
            update,
            body: Box::new(body),
        })
    }

    fn parse_for_head_variable_declaration(
        &mut self,
        kind: BindingKind,
    ) -> Result<ForInitializer, ParseError> {
        let saved_allow_in = self.allow_in;
        self.allow_in = false;
        let declaration = self.parse_variable_declaration(kind);
        self.allow_in = saved_allow_in;
        let declaration = declaration?;
        match declaration {
            Stmt::VariableDeclaration(declaration) => {
                Ok(ForInitializer::VariableDeclaration(declaration))
            }
            Stmt::VariableDeclarations(declarations) => {
                Ok(ForInitializer::VariableDeclarations(declarations))
            }
            _ => Err(ParseError {
                message: "invalid for initializer".to_string(),
                position: self.current_position(),
            }),
        }
    }

    fn parse_for_in_of_rhs_expression(&mut self) -> Result<Expr, ParseError> {
        let mut expressions = vec![self.parse_expression_inner()?];
        while self.matches(&TokenKind::Comma) {
            expressions.push(self.parse_expression_inner()?);
        }
        if expressions.len() == 1 {
            Ok(expressions
                .into_iter()
                .next()
                .expect("for-in/of rhs expression should exist"))
        } else {
            Ok(Expr::Sequence(expressions))
        }
    }

    fn parse_for_head_array_pattern_after_lbracket(
        &mut self,
        kind: BindingKind,
    ) -> Result<Vec<ForHeadArrayPatternElement>, ParseError> {
        let mut elements = Vec::new();
        let mut element_index = 0usize;
        while !self.check(&TokenKind::RBracket) {
            if self.matches(&TokenKind::Comma) {
                element_index += 1;
                continue;
            }
            let name = if kind == BindingKind::Var {
                self.expect_var_binding_identifier("expected binding name")?
            } else {
                self.expect_binding_identifier("expected binding name")?
            };
            let default_initializer = if self.matches(&TokenKind::Equal) {
                Some(self.parse_expression_inner()?)
            } else {
                None
            };
            elements.push(ForHeadArrayPatternElement {
                index: element_index,
                name: Identifier(name),
                default_initializer,
            });
            element_index += 1;
            if self.check(&TokenKind::RBracket) {
                break;
            }
            self.expect(TokenKind::Comma, "expected ',' in array binding pattern")?;
        }
        self.expect(
            TokenKind::RBracket,
            "expected ']' after array binding pattern",
        )?;
        if elements.is_empty() {
            return Err(self.error_current("expected binding name"));
        }
        Ok(elements)
    }

    fn parse_for_in_of_array_pattern_statement(
        &mut self,
        kind: BindingKind,
        elements: Vec<ForHeadArrayPatternElement>,
    ) -> Result<Stmt, ParseError> {
        let loop_kind = if self.matches_keyword("in") {
            "in"
        } else if self.matches_keyword("of") {
            "of"
        } else {
            return Err(self.error_current("expected 'in' or 'of' after array binding pattern"));
        };
        let iterable = self.parse_for_in_of_rhs_expression()?;
        self.expect(TokenKind::RParen, "expected ')' after for-in/of clauses")?;

        self.loop_depth += 1;
        self.breakable_depth += 1;
        let body = self.parse_embedded_statement(false);
        self.loop_depth = self.loop_depth.saturating_sub(1);
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
        let body = body?;

        if loop_kind == "in" {
            self.lower_for_in_array_pattern_statement(kind, elements, iterable, body)
        } else {
            self.lower_for_of_array_pattern_statement(kind, elements, iterable, body)
        }
    }

    fn parse_for_head_array_pattern_declaration(
        &mut self,
        kind: BindingKind,
        elements: Vec<ForHeadArrayPatternElement>,
    ) -> Result<Vec<VariableDeclaration>, ParseError> {
        self.expect(TokenKind::Equal, "expected '=' after array binding pattern")?;
        let source = self.parse_expression_no_in()?;
        let source_name = self.next_for_in_temp_identifier("array_head");
        let mut declarations = vec![VariableDeclaration {
            kind,
            name: source_name.clone(),
            initializer: Some(source),
        }];
        for element in elements {
            let mut read = Expr::MemberComputed {
                object: Box::new(Expr::Identifier(source_name.clone())),
                property: Box::new(Expr::Number(element.index as f64)),
            };
            if let Some(default_initializer) = element.default_initializer {
                let condition_read = read.clone();
                read = Expr::Conditional {
                    condition: Box::new(Expr::Binary {
                        op: BinaryOp::StrictEqual,
                        left: Box::new(condition_read),
                        right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                    }),
                    consequent: Box::new(default_initializer),
                    alternate: Box::new(read),
                };
            }
            declarations.push(VariableDeclaration {
                kind,
                name: element.name,
                initializer: Some(read),
            });
        }
        Ok(declarations)
    }

    fn parse_for_head_object_pattern_declaration(
        &mut self,
        kind: BindingKind,
        binding_name: Identifier,
        effects: Vec<Stmt>,
    ) -> Result<Vec<VariableDeclaration>, ParseError> {
        self.expect(
            TokenKind::Equal,
            "expected '=' after object binding pattern",
        )?;
        let source = self.parse_expression_no_in()?;
        let mut declarations = vec![VariableDeclaration {
            kind,
            name: binding_name,
            initializer: Some(source),
        }];
        declarations.extend(self.lower_object_pattern_variable_declarations(kind, effects)?);
        Ok(declarations)
    }

    fn parse_for_in_of_object_pattern_statement(
        &mut self,
        kind: BindingKind,
        binding_name: Identifier,
        effects: Vec<Stmt>,
    ) -> Result<Stmt, ParseError> {
        let loop_kind = if self.matches_keyword("in") {
            "in"
        } else if self.matches_keyword("of") {
            "of"
        } else {
            return Err(self.error_current("expected 'in' or 'of' after object binding pattern"));
        };
        let iterable = self.parse_for_in_of_rhs_expression()?;
        self.expect(TokenKind::RParen, "expected ')' after for-in/of clauses")?;

        self.loop_depth += 1;
        self.breakable_depth += 1;
        let body = self.parse_embedded_statement(false);
        self.loop_depth = self.loop_depth.saturating_sub(1);
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
        let body = body?;

        if loop_kind == "in" {
            self.lower_for_in_object_pattern_statement(kind, binding_name, effects, iterable, body)
        } else {
            self.lower_for_of_object_pattern_statement(kind, binding_name, effects, iterable, body)
        }
    }

    fn for_head_starts_with_lexical_let_declaration(&self) -> bool {
        if !self.check_keyword("let") {
            return false;
        }
        if self.check_next(&TokenKind::LBracket) || self.check_next(&TokenKind::LBrace) {
            return true;
        }
        if !matches!(
            self.tokens.get(self.pos + 1).map(|token| &token.kind),
            Some(TokenKind::Identifier(_))
        ) {
            return false;
        }
        matches!(
            self.tokens.get(self.pos + 2).map(|token| &token.kind),
            Some(TokenKind::Equal | TokenKind::Comma | TokenKind::Semicolon)
        ) || self.check_nth_keyword(2, "in")
            || self.check_nth_keyword(2, "of")
    }

    fn statement_starts_with_lexical_let_declaration(&self) -> bool {
        if !self.check_keyword("let") {
            return false;
        }
        self.check_next(&TokenKind::LBracket)
            || self.check_next(&TokenKind::LBrace)
            || matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                Some(TokenKind::Identifier(_))
            )
    }

    fn supports_for_in_lowering(initializer: Option<&ForInitializer>) -> bool {
        match initializer {
            Some(ForInitializer::VariableDeclaration(declaration)) => {
                declaration.kind == BindingKind::Var || declaration.initializer.is_none()
            }
            Some(ForInitializer::VariableDeclarations(declarations)) => {
                declarations.len() == 1
                    && (declarations[0].kind == BindingKind::Var
                        || declarations[0].initializer.is_none())
            }
            Some(ForInitializer::Expression(target)) => {
                Self::is_simple_assignment_target_expression(target)
                    || Self::is_annex_b_call_assignment_target(target)
            }
            None => false,
        }
    }

    fn supports_for_of_lowering(initializer: Option<&ForInitializer>) -> bool {
        match initializer {
            Some(ForInitializer::VariableDeclaration(VariableDeclaration {
                initializer, ..
            })) => initializer.is_none(),
            Some(ForInitializer::VariableDeclarations(declarations)) => {
                declarations.len() == 1 && declarations[0].initializer.is_none()
            }
            Some(ForInitializer::Expression(target)) => {
                Self::is_simple_assignment_target_expression(target)
                    || Self::is_annex_b_call_assignment_target(target)
            }
            None => false,
        }
    }

    fn is_simple_assignment_target_expression(expression: &Expr) -> bool {
        matches!(
            expression,
            Expr::Identifier(_) | Expr::Member { .. } | Expr::MemberComputed { .. }
        )
    }

    fn is_annex_b_call_assignment_target(expression: &Expr) -> bool {
        matches!(expression, Expr::Call { .. })
    }

    fn lower_for_in_statement(
        &mut self,
        initializer: Option<ForInitializer>,
        iterable: Expr,
        body: Stmt,
    ) -> Result<Stmt, ParseError> {
        let iterable_name = self.next_for_in_temp_identifier("iterable");
        let keys_name = self.next_for_in_temp_identifier("keys");
        let index_name = self.next_for_in_temp_identifier("index");
        let current_name = self.next_for_in_temp_identifier("current");
        let iterable_identifier = Expr::Identifier(iterable_name.clone());
        let current_key_value_expr = Expr::Identifier(current_name.clone());

        let mut statements = Self::for_initializer_tdz_names(initializer.as_ref())
            .into_iter()
            .map(Self::tdz_marker_declaration)
            .collect::<Vec<_>>();
        let mut initializer_prelude = Vec::new();
        let mut iteration_statements = Vec::new();
        match initializer {
            None => {}
            Some(ForInitializer::VariableDeclaration(declaration)) => {
                self.lower_for_in_initializer_declaration(
                    declaration,
                    &current_key_value_expr,
                    &mut initializer_prelude,
                    &mut iteration_statements,
                )?;
            }
            Some(ForInitializer::VariableDeclarations(declarations)) => {
                if declarations.len() != 1 {
                    return Err(self.error_current("invalid for-in initializer"));
                }
                let declaration = declarations
                    .into_iter()
                    .next()
                    .expect("for-in declaration should exist");
                self.lower_for_in_initializer_declaration(
                    declaration,
                    &current_key_value_expr,
                    &mut initializer_prelude,
                    &mut iteration_statements,
                )?;
            }
            Some(ForInitializer::Expression(target)) => {
                let assignment = self.rewrite_assignment_target(
                    target,
                    current_key_value_expr.clone(),
                    AssignmentKind::Simple,
                    self.current_position(),
                )?;
                iteration_statements.push(Stmt::Expression(Expr::Unary {
                    op: UnaryOp::Void,
                    expr: Box::new(assignment),
                }));
            }
        }

        statements.extend(initializer_prelude);
        statements.extend(vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterable_name.clone(),
                initializer: Some(iterable),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: keys_name.clone(),
                initializer: Some(Expr::Conditional {
                    condition: Box::new(Expr::Binary {
                        op: BinaryOp::LogicalOr,
                        left: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(iterable_identifier.clone()),
                            right: Box::new(Expr::Null),
                        }),
                        right: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(iterable_identifier.clone()),
                            right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                        }),
                    }),
                    consequent: Box::new(Expr::ArrayLiteral(vec![])),
                    alternate: Box::new(Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                            property: "__forInKeys".to_string(),
                        }),
                        arguments: vec![iterable_identifier],
                    }),
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: index_name.clone(),
                initializer: Some(Expr::Number(0.0)),
            }),
        ]);

        let current_key_expr = Expr::MemberComputed {
            object: Box::new(Expr::Identifier(keys_name.clone())),
            property: Box::new(Expr::Identifier(index_name.clone())),
        };

        let current_declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: current_name,
            initializer: Some(current_key_expr),
        });

        iteration_statements.push(body);
        let guarded_iteration_body = Stmt::If {
            condition: Expr::Binary {
                op: BinaryOp::In,
                left: Box::new(current_key_value_expr),
                right: Box::new(Expr::Identifier(iterable_name.clone())),
            },
            consequent: Box::new(Stmt::Block(iteration_statements)),
            alternate: None,
        };

        let condition = Expr::Binary {
            op: BinaryOp::Less,
            left: Box::new(Expr::Identifier(index_name.clone())),
            right: Box::new(Expr::Member {
                object: Box::new(Expr::Identifier(keys_name.clone())),
                property: "length".to_string(),
            }),
        };
        let update = Expr::Assign {
            target: index_name.clone(),
            value: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Identifier(index_name)),
                right: Box::new(Expr::Number(1.0)),
            }),
        };

        statements.push(Stmt::For {
            initializer: None,
            condition: Some(condition),
            update: Some(update),
            body: Box::new(Stmt::Block(vec![
                current_declaration,
                guarded_iteration_body,
            ])),
        });

        Ok(Stmt::Block(statements))
    }

    fn lower_for_of_statement(
        &mut self,
        initializer: Option<ForInitializer>,
        iterable: Expr,
        body: Stmt,
    ) -> Result<Stmt, ParseError> {
        let iterable_name = self.next_for_in_temp_identifier("iterable");
        let iterator_name = self.next_for_in_temp_identifier("iterator");
        let done_name = self.next_for_in_temp_identifier("done");
        let step_name = self.next_for_in_temp_identifier("step");
        let current_name = self.next_for_in_temp_identifier("current");
        let close_name = self.next_for_in_temp_identifier("close");

        let mut statements = Self::for_initializer_tdz_names(initializer.as_ref())
            .into_iter()
            .map(Self::tdz_marker_declaration)
            .collect::<Vec<_>>();
        statements.extend(vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterable_name.clone(),
                initializer: Some(iterable),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterator_name.clone(),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfIterator".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterable_name)],
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: done_name.clone(),
                initializer: Some(Expr::Bool(false)),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: step_name.clone(),
                initializer: Some(Expr::Identifier(Identifier("undefined".to_string()))),
            }),
        ]);

        let current_identifier_expr = Expr::Identifier(current_name.clone());

        let mut iteration_statements = Vec::new();
        match initializer {
            None => {}
            Some(ForInitializer::VariableDeclaration(declaration)) => {
                self.lower_for_in_initializer_declaration(
                    declaration,
                    &current_identifier_expr,
                    &mut statements,
                    &mut iteration_statements,
                )?;
            }
            Some(ForInitializer::VariableDeclarations(declarations)) => {
                if declarations.len() != 1 || declarations[0].initializer.is_some() {
                    return Err(self.error_current("invalid for-of initializer"));
                }
                let declaration = declarations
                    .into_iter()
                    .next()
                    .expect("for-of declaration should exist");
                self.lower_for_in_initializer_declaration(
                    declaration,
                    &current_identifier_expr,
                    &mut statements,
                    &mut iteration_statements,
                )?;
            }
            Some(ForInitializer::Expression(target)) => {
                let assignment = self.rewrite_assignment_target(
                    target,
                    current_identifier_expr.clone(),
                    AssignmentKind::Simple,
                    self.current_position(),
                )?;
                iteration_statements.push(Stmt::Expression(Expr::Unary {
                    op: UnaryOp::Void,
                    expr: Box::new(assignment),
                }));
            }
        }

        iteration_statements.push(body);

        let current_declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: current_name.clone(),
            initializer: Some(Expr::Member {
                object: Box::new(Expr::Identifier(step_name.clone())),
                property: "value".to_string(),
            }),
        });
        let condition = Expr::Sequence(vec![
            Expr::Assign {
                target: step_name.clone(),
                value: Box::new(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfStep".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterator_name.clone())],
                }),
            },
            Expr::Assign {
                target: done_name.clone(),
                value: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(step_name.clone())),
                    property: "done".to_string(),
                }),
            },
            Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name.clone())),
            },
        ]);
        let loop_body = Stmt::Block(vec![current_declaration, Stmt::Block(iteration_statements)]);
        let try_block = vec![Stmt::For {
            initializer: None,
            condition: Some(condition),
            update: None,
            body: Box::new(loop_body),
        }];
        let finally_block = vec![Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name)),
            },
            consequent: Box::new(Stmt::Block(vec![Stmt::VariableDeclaration(
                VariableDeclaration {
                    kind: BindingKind::Let,
                    name: close_name,
                    initializer: Some(Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                            property: "__forOfClose".to_string(),
                        }),
                        arguments: vec![Expr::Identifier(iterator_name)],
                    }),
                },
            )])),
            alternate: None,
        }];
        statements.push(Stmt::Try {
            try_block,
            catch_param: None,
            catch_block: None,
            finally_block: Some(finally_block),
        });

        Ok(Stmt::Block(statements))
    }

    fn lower_for_await_of_statement(
        &mut self,
        initializer: Option<ForInitializer>,
        iterable: Expr,
        body: Stmt,
    ) -> Result<Stmt, ParseError> {
        let iterable_name = self.next_for_in_temp_identifier("iterable");
        let iterator_name = self.next_for_in_temp_identifier("iterator");
        let done_name = self.next_for_in_temp_identifier("done");
        let step_name = self.next_for_in_temp_identifier("step");
        let current_name = self.next_for_in_temp_identifier("current");
        let close_name = self.next_for_in_temp_identifier("close");

        let mut statements = Self::for_initializer_tdz_names(initializer.as_ref())
            .into_iter()
            .map(Self::tdz_marker_declaration)
            .collect::<Vec<_>>();
        statements.extend(vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterable_name.clone(),
                initializer: Some(iterable),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterator_name.clone(),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forAwaitIterator".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterable_name)],
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: done_name.clone(),
                initializer: Some(Expr::Bool(false)),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: step_name.clone(),
                initializer: Some(Expr::Identifier(Identifier("undefined".to_string()))),
            }),
        ]);

        let current_identifier_expr = Expr::Identifier(current_name.clone());

        let mut iteration_statements = Vec::new();
        match initializer {
            None => {}
            Some(ForInitializer::VariableDeclaration(declaration)) => {
                self.lower_for_in_initializer_declaration(
                    declaration,
                    &current_identifier_expr,
                    &mut statements,
                    &mut iteration_statements,
                )?;
            }
            Some(ForInitializer::VariableDeclarations(declarations)) => {
                if declarations.len() != 1 || declarations[0].initializer.is_some() {
                    return Err(self.error_current("invalid for-await-of initializer"));
                }
                let declaration = declarations
                    .into_iter()
                    .next()
                    .expect("for-await-of declaration should exist");
                self.lower_for_in_initializer_declaration(
                    declaration,
                    &current_identifier_expr,
                    &mut statements,
                    &mut iteration_statements,
                )?;
            }
            Some(ForInitializer::Expression(target)) => {
                let assignment = self.rewrite_assignment_target(
                    target,
                    current_identifier_expr.clone(),
                    AssignmentKind::Simple,
                    self.current_position(),
                )?;
                iteration_statements.push(Stmt::Expression(Expr::Unary {
                    op: UnaryOp::Void,
                    expr: Box::new(assignment),
                }));
            }
        }

        iteration_statements.push(body);

        let current_declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: current_name.clone(),
            initializer: Some(Expr::Member {
                object: Box::new(Expr::Identifier(step_name.clone())),
                property: "value".to_string(),
            }),
        });
        let condition = Expr::Sequence(vec![
            Expr::Assign {
                target: step_name.clone(),
                value: Box::new(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfStep".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterator_name.clone())],
                }),
            },
            Expr::Assign {
                target: done_name.clone(),
                value: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(step_name.clone())),
                    property: "done".to_string(),
                }),
            },
            Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name.clone())),
            },
        ]);
        let loop_body = Stmt::Block(vec![current_declaration, Stmt::Block(iteration_statements)]);
        let try_block = vec![Stmt::For {
            initializer: None,
            condition: Some(condition),
            update: None,
            body: Box::new(loop_body),
        }];
        let finally_block = vec![Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name)),
            },
            consequent: Box::new(Stmt::Block(vec![Stmt::VariableDeclaration(
                VariableDeclaration {
                    kind: BindingKind::Let,
                    name: close_name,
                    initializer: Some(Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                            property: "__forOfClose".to_string(),
                        }),
                        arguments: vec![Expr::Identifier(iterator_name)],
                    }),
                },
            )])),
            alternate: None,
        }];
        statements.push(Stmt::Try {
            try_block,
            catch_param: None,
            catch_block: None,
            finally_block: Some(finally_block),
        });

        Ok(Stmt::Block(statements))
    }

    fn lower_for_in_array_pattern_statement(
        &mut self,
        kind: BindingKind,
        elements: Vec<ForHeadArrayPatternElement>,
        iterable: Expr,
        body: Stmt,
    ) -> Result<Stmt, ParseError> {
        let iterable_name = self.next_for_in_temp_identifier("iterable");
        let keys_name = self.next_for_in_temp_identifier("keys");
        let index_name = self.next_for_in_temp_identifier("index");
        let current_name = self.next_for_in_temp_identifier("current");
        let iterable_identifier = Expr::Identifier(iterable_name.clone());

        let mut statements = if matches!(kind, BindingKind::Let | BindingKind::Const) {
            elements
                .iter()
                .map(|element| Self::tdz_marker_declaration(element.name.clone()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        statements.extend(vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterable_name.clone(),
                initializer: Some(Expr::Conditional {
                    condition: Box::new(Expr::Binary {
                        op: BinaryOp::LogicalOr,
                        left: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(iterable.clone()),
                            right: Box::new(Expr::Null),
                        }),
                        right: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(iterable.clone()),
                            right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                        }),
                    }),
                    consequent: Box::new(Expr::ObjectLiteral(Vec::new())),
                    alternate: Box::new(iterable),
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: keys_name.clone(),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forInKeys".to_string(),
                    }),
                    arguments: vec![iterable_identifier],
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: index_name.clone(),
                initializer: Some(Expr::Number(0.0)),
            }),
        ]);

        let current_key_expr = Expr::MemberComputed {
            object: Box::new(Expr::Identifier(keys_name.clone())),
            property: Box::new(Expr::Identifier(index_name.clone())),
        };
        let current_key_value_expr = Expr::Identifier(current_name.clone());

        let current_declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: current_name,
            initializer: Some(current_key_expr),
        });
        let mut iteration_statements = Vec::new();
        self.lower_array_pattern_bindings(
            kind,
            &elements,
            &current_key_value_expr,
            &mut statements,
            &mut iteration_statements,
        );
        iteration_statements.push(body);

        let guarded_iteration_body = Stmt::If {
            condition: Expr::Binary {
                op: BinaryOp::In,
                left: Box::new(current_key_value_expr),
                right: Box::new(Expr::Identifier(iterable_name.clone())),
            },
            consequent: Box::new(Stmt::Block(iteration_statements)),
            alternate: None,
        };

        let condition = Expr::Binary {
            op: BinaryOp::Less,
            left: Box::new(Expr::Identifier(index_name.clone())),
            right: Box::new(Expr::Member {
                object: Box::new(Expr::Identifier(keys_name)),
                property: "length".to_string(),
            }),
        };
        let update = Expr::Assign {
            target: index_name.clone(),
            value: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Identifier(index_name)),
                right: Box::new(Expr::Number(1.0)),
            }),
        };

        statements.push(Stmt::For {
            initializer: None,
            condition: Some(condition),
            update: Some(update),
            body: Box::new(Stmt::Block(vec![
                current_declaration,
                guarded_iteration_body,
            ])),
        });

        Ok(Stmt::Block(statements))
    }

    fn lower_for_of_array_pattern_statement(
        &mut self,
        kind: BindingKind,
        elements: Vec<ForHeadArrayPatternElement>,
        iterable: Expr,
        body: Stmt,
    ) -> Result<Stmt, ParseError> {
        let iterable_name = self.next_for_in_temp_identifier("iterable");
        let iterator_name = self.next_for_in_temp_identifier("iterator");
        let done_name = self.next_for_in_temp_identifier("done");
        let step_name = self.next_for_in_temp_identifier("step");
        let current_name = self.next_for_in_temp_identifier("current");
        let close_name = self.next_for_in_temp_identifier("close");

        let mut statements = if matches!(kind, BindingKind::Let | BindingKind::Const) {
            elements
                .iter()
                .map(|element| Self::tdz_marker_declaration(element.name.clone()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        statements.extend(vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterable_name.clone(),
                initializer: Some(iterable),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterator_name.clone(),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfIterator".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterable_name)],
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: done_name.clone(),
                initializer: Some(Expr::Bool(false)),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: step_name.clone(),
                initializer: Some(Expr::Identifier(Identifier("undefined".to_string()))),
            }),
        ]);

        let current_declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: current_name.clone(),
            initializer: Some(Expr::Member {
                object: Box::new(Expr::Identifier(step_name.clone())),
                property: "value".to_string(),
            }),
        });
        let current_identifier_expr = Expr::Identifier(current_name);

        let mut iteration_statements = Vec::new();
        self.lower_array_pattern_bindings(
            kind,
            &elements,
            &current_identifier_expr,
            &mut statements,
            &mut iteration_statements,
        );
        iteration_statements.push(body);

        let condition = Expr::Sequence(vec![
            Expr::Assign {
                target: step_name.clone(),
                value: Box::new(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfStep".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterator_name.clone())],
                }),
            },
            Expr::Assign {
                target: done_name.clone(),
                value: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(step_name.clone())),
                    property: "done".to_string(),
                }),
            },
            Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name.clone())),
            },
        ]);
        let loop_body = Stmt::Block(vec![current_declaration, Stmt::Block(iteration_statements)]);
        let try_block = vec![Stmt::For {
            initializer: None,
            condition: Some(condition),
            update: None,
            body: Box::new(loop_body),
        }];
        let finally_block = vec![Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name)),
            },
            consequent: Box::new(Stmt::Block(vec![Stmt::VariableDeclaration(
                VariableDeclaration {
                    kind: BindingKind::Let,
                    name: close_name,
                    initializer: Some(Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                            property: "__forOfClose".to_string(),
                        }),
                        arguments: vec![Expr::Identifier(iterator_name)],
                    }),
                },
            )])),
            alternate: None,
        }];
        statements.push(Stmt::Try {
            try_block,
            catch_param: None,
            catch_block: None,
            finally_block: Some(finally_block),
        });

        Ok(Stmt::Block(statements))
    }

    fn lower_for_in_object_pattern_statement(
        &mut self,
        kind: BindingKind,
        binding_name: Identifier,
        effects: Vec<Stmt>,
        iterable: Expr,
        body: Stmt,
    ) -> Result<Stmt, ParseError> {
        let iterable_name = self.next_for_in_temp_identifier("iterable");
        let keys_name = self.next_for_in_temp_identifier("keys");
        let index_name = self.next_for_in_temp_identifier("index");
        let current_name = self.next_for_in_temp_identifier("current");
        let iterable_identifier = Expr::Identifier(iterable_name.clone());

        let binding_targets = Self::object_pattern_binding_targets(&effects);
        let mut statements = if matches!(kind, BindingKind::Let | BindingKind::Const) {
            binding_targets
                .iter()
                .cloned()
                .map(Self::tdz_marker_declaration)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        statements.extend(vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterable_name.clone(),
                initializer: Some(Expr::Conditional {
                    condition: Box::new(Expr::Binary {
                        op: BinaryOp::LogicalOr,
                        left: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(iterable.clone()),
                            right: Box::new(Expr::Null),
                        }),
                        right: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(iterable.clone()),
                            right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                        }),
                    }),
                    consequent: Box::new(Expr::ObjectLiteral(Vec::new())),
                    alternate: Box::new(iterable),
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: keys_name.clone(),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forInKeys".to_string(),
                    }),
                    arguments: vec![iterable_identifier],
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: index_name.clone(),
                initializer: Some(Expr::Number(0.0)),
            }),
        ]);

        let current_key_expr = Expr::MemberComputed {
            object: Box::new(Expr::Identifier(keys_name.clone())),
            property: Box::new(Expr::Identifier(index_name.clone())),
        };
        let current_key_value_expr = Expr::Identifier(current_name.clone());

        let current_declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: current_name,
            initializer: Some(current_key_expr),
        });
        let mut iteration_statements = Vec::new();
        self.lower_object_pattern_bindings(
            kind,
            binding_name,
            &effects,
            &current_key_value_expr,
            &mut statements,
            &mut iteration_statements,
        );
        iteration_statements.push(body);

        let guarded_iteration_body = Stmt::If {
            condition: Expr::Binary {
                op: BinaryOp::In,
                left: Box::new(current_key_value_expr),
                right: Box::new(Expr::Identifier(iterable_name.clone())),
            },
            consequent: Box::new(Stmt::Block(iteration_statements)),
            alternate: None,
        };

        let condition = Expr::Binary {
            op: BinaryOp::Less,
            left: Box::new(Expr::Identifier(index_name.clone())),
            right: Box::new(Expr::Member {
                object: Box::new(Expr::Identifier(keys_name)),
                property: "length".to_string(),
            }),
        };
        let update = Expr::Assign {
            target: index_name.clone(),
            value: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Identifier(index_name)),
                right: Box::new(Expr::Number(1.0)),
            }),
        };

        statements.push(Stmt::For {
            initializer: None,
            condition: Some(condition),
            update: Some(update),
            body: Box::new(Stmt::Block(vec![
                current_declaration,
                guarded_iteration_body,
            ])),
        });

        Ok(Stmt::Block(statements))
    }

    fn lower_for_of_object_pattern_statement(
        &mut self,
        kind: BindingKind,
        binding_name: Identifier,
        effects: Vec<Stmt>,
        iterable: Expr,
        body: Stmt,
    ) -> Result<Stmt, ParseError> {
        let iterable_name = self.next_for_in_temp_identifier("iterable");
        let iterator_name = self.next_for_in_temp_identifier("iterator");
        let done_name = self.next_for_in_temp_identifier("done");
        let step_name = self.next_for_in_temp_identifier("step");
        let current_name = self.next_for_in_temp_identifier("current");
        let close_name = self.next_for_in_temp_identifier("close");

        let binding_targets = Self::object_pattern_binding_targets(&effects);
        let mut statements = if matches!(kind, BindingKind::Let | BindingKind::Const) {
            binding_targets
                .iter()
                .cloned()
                .map(Self::tdz_marker_declaration)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        statements.extend(vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterable_name.clone(),
                initializer: Some(iterable),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: iterator_name.clone(),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfIterator".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterable_name)],
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: done_name.clone(),
                initializer: Some(Expr::Bool(false)),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: step_name.clone(),
                initializer: Some(Expr::Identifier(Identifier("undefined".to_string()))),
            }),
        ]);

        let current_declaration = Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: current_name.clone(),
            initializer: Some(Expr::Member {
                object: Box::new(Expr::Identifier(step_name.clone())),
                property: "value".to_string(),
            }),
        });
        let current_identifier_expr = Expr::Identifier(current_name);

        let mut iteration_statements = Vec::new();
        self.lower_object_pattern_bindings(
            kind,
            binding_name,
            &effects,
            &current_identifier_expr,
            &mut statements,
            &mut iteration_statements,
        );
        iteration_statements.push(body);

        let condition = Expr::Sequence(vec![
            Expr::Assign {
                target: step_name.clone(),
                value: Box::new(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfStep".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(iterator_name.clone())],
                }),
            },
            Expr::Assign {
                target: done_name.clone(),
                value: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(step_name.clone())),
                    property: "done".to_string(),
                }),
            },
            Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name.clone())),
            },
        ]);
        let loop_body = Stmt::Block(vec![current_declaration, Stmt::Block(iteration_statements)]);
        let try_block = vec![Stmt::For {
            initializer: None,
            condition: Some(condition),
            update: None,
            body: Box::new(loop_body),
        }];
        let finally_block = vec![Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(Expr::Identifier(done_name)),
            },
            consequent: Box::new(Stmt::Block(vec![Stmt::VariableDeclaration(
                VariableDeclaration {
                    kind: BindingKind::Let,
                    name: close_name,
                    initializer: Some(Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                            property: "__forOfClose".to_string(),
                        }),
                        arguments: vec![Expr::Identifier(iterator_name)],
                    }),
                },
            )])),
            alternate: None,
        }];
        statements.push(Stmt::Try {
            try_block,
            catch_param: None,
            catch_block: None,
            finally_block: Some(finally_block),
        });

        Ok(Stmt::Block(statements))
    }

    fn lower_array_pattern_bindings(
        &mut self,
        kind: BindingKind,
        elements: &[ForHeadArrayPatternElement],
        current_value_expr: &Expr,
        outer_statements: &mut Vec<Stmt>,
        iteration_statements: &mut Vec<Stmt>,
    ) {
        if kind == BindingKind::Var {
            for element in elements {
                outer_statements.push(Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Var,
                    name: element.name.clone(),
                    initializer: None,
                }));
            }
        }

        for element in elements {
            let value = Self::array_pattern_element_value_expr(current_value_expr, element);
            if kind == BindingKind::Var {
                iteration_statements.push(Stmt::Expression(Expr::Unary {
                    op: UnaryOp::Void,
                    expr: Box::new(Expr::Assign {
                        target: element.name.clone(),
                        value: Box::new(value),
                    }),
                }));
            } else {
                iteration_statements.push(Stmt::VariableDeclaration(VariableDeclaration {
                    kind,
                    name: element.name.clone(),
                    initializer: Some(value),
                }));
            }
        }
    }

    fn object_pattern_binding_targets(effects: &[Stmt]) -> Vec<Identifier> {
        effects
            .iter()
            .filter_map(|statement| match statement {
                Stmt::Expression(Expr::Assign { target, .. }) => Some(target.clone()),
                _ => None,
            })
            .collect()
    }

    fn lower_object_pattern_bindings(
        &mut self,
        kind: BindingKind,
        binding_name: Identifier,
        effects: &[Stmt],
        current_value_expr: &Expr,
        outer_statements: &mut Vec<Stmt>,
        iteration_statements: &mut Vec<Stmt>,
    ) {
        if kind == BindingKind::Var {
            for name in Self::object_pattern_binding_targets(effects) {
                outer_statements.push(Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Var,
                    name,
                    initializer: None,
                }));
            }
        }

        iteration_statements.push(Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name: binding_name,
            initializer: Some(current_value_expr.clone()),
        }));

        for effect in effects {
            match effect {
                Stmt::Expression(Expr::Assign { target, value }) => {
                    if kind == BindingKind::Var {
                        iteration_statements.push(Stmt::Expression(Expr::Unary {
                            op: UnaryOp::Void,
                            expr: Box::new(Expr::Assign {
                                target: target.clone(),
                                value: value.clone(),
                            }),
                        }));
                    } else {
                        iteration_statements.push(Stmt::VariableDeclaration(VariableDeclaration {
                            kind,
                            name: target.clone(),
                            initializer: Some((**value).clone()),
                        }));
                    }
                }
                other => iteration_statements.push(other.clone()),
            }
        }
    }

    fn array_pattern_element_value_expr(
        current_value_expr: &Expr,
        element: &ForHeadArrayPatternElement,
    ) -> Expr {
        let extracted = Expr::MemberComputed {
            object: Box::new(current_value_expr.clone()),
            property: Box::new(Expr::Number(element.index as f64)),
        };
        if let Some(default_initializer) = &element.default_initializer {
            Expr::Conditional {
                condition: Box::new(Expr::Binary {
                    op: BinaryOp::StrictEqual,
                    left: Box::new(extracted.clone()),
                    right: Box::new(Expr::Unary {
                        op: UnaryOp::Void,
                        expr: Box::new(Expr::Number(0.0)),
                    }),
                }),
                consequent: Box::new(default_initializer.clone()),
                alternate: Box::new(extracted),
            }
        } else {
            extracted
        }
    }

    fn lower_for_in_initializer_declaration(
        &mut self,
        declaration: VariableDeclaration,
        current_key_expr: &Expr,
        outer_statements: &mut Vec<Stmt>,
        loop_statements: &mut Vec<Stmt>,
    ) -> Result<(), ParseError> {
        if declaration.kind == BindingKind::Var {
            outer_statements.push(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Var,
                name: declaration.name.clone(),
                initializer: declaration.initializer,
            }));
            loop_statements.push(Stmt::Expression(Expr::Unary {
                op: UnaryOp::Void,
                expr: Box::new(Expr::Assign {
                    target: declaration.name,
                    value: Box::new(current_key_expr.clone()),
                }),
            }));
            return Ok(());
        }
        if declaration.initializer.is_some() {
            return Err(self.error_current("invalid for-in initializer"));
        }
        loop_statements.push(Stmt::VariableDeclaration(VariableDeclaration {
            kind: declaration.kind,
            name: declaration.name,
            initializer: Some(current_key_expr.clone()),
        }));
        Ok(())
    }

    fn for_initializer_tdz_names(initializer: Option<&ForInitializer>) -> Vec<Identifier> {
        match initializer {
            Some(ForInitializer::VariableDeclaration(declaration))
                if matches!(declaration.kind, BindingKind::Let | BindingKind::Const) =>
            {
                vec![declaration.name.clone()]
            }
            Some(ForInitializer::VariableDeclarations(declarations)) => declarations
                .iter()
                .filter(|declaration| {
                    matches!(declaration.kind, BindingKind::Let | BindingKind::Const)
                })
                .map(|declaration| declaration.name.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    fn tdz_marker_declaration(name: Identifier) -> Stmt {
        Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name,
            initializer: Some(Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                    property: "__tdzMarker".to_string(),
                }),
                arguments: Vec::new(),
            }),
        })
    }

    fn next_for_in_temp_identifier(&mut self, category: &str) -> Identifier {
        let name = format!("$__for_in_{category}_{}", self.class_temp_index);
        self.class_temp_index += 1;
        Identifier(name)
    }

    fn next_catch_temp_identifier(&mut self) -> Identifier {
        let name = format!("$__catch_param_{}", self.class_temp_index);
        self.class_temp_index += 1;
        Identifier(name)
    }

    fn parse_with_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'with'")?;
        let object = self.parse_expression_with_commas()?;
        self.expect(TokenKind::RParen, "expected ')' after with object")?;
        let body = self.parse_embedded_statement(false)?;
        Ok(Stmt::With {
            object,
            body: Box::new(body),
        })
    }

    fn parse_switch_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'switch'")?;
        let discriminant = self.parse_expression_with_commas()?;
        self.expect(TokenKind::RParen, "expected ')' after switch discriminant")?;
        self.expect(TokenKind::LBrace, "expected '{' before switch body")?;

        self.breakable_depth += 1;
        let cases = self.parse_switch_cases();
        self.breakable_depth = self.breakable_depth.saturating_sub(1);
        let cases = cases?;

        self.expect(TokenKind::RBrace, "expected '}' after switch body")?;
        Ok(Stmt::Switch {
            discriminant,
            cases,
        })
    }

    fn parse_switch_cases(&mut self) -> Result<Vec<SwitchCase>, ParseError> {
        let mut cases = Vec::new();
        let mut has_default = false;
        while !self.check(&TokenKind::RBrace) {
            if self.matches_keyword("case") {
                let test = self.parse_expression_with_commas()?;
                self.expect(TokenKind::Colon, "expected ':' after case label")?;
                let consequent = self.parse_switch_case_consequent()?;
                cases.push(SwitchCase {
                    test: Some(test),
                    consequent,
                });
                continue;
            }

            if self.matches_keyword("default") {
                if has_default {
                    return Err(self.error_current("duplicate default in switch"));
                }
                has_default = true;
                self.expect(TokenKind::Colon, "expected ':' after default label")?;
                let consequent = self.parse_switch_case_consequent()?;
                cases.push(SwitchCase {
                    test: None,
                    consequent,
                });
                continue;
            }

            if self.is_eof() {
                break;
            }
            return Err(self.error_current("expected 'case' or 'default' in switch body"));
        }
        Ok(cases)
    }

    fn parse_switch_case_consequent(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();
        loop {
            if self.check(&TokenKind::RBrace)
                || self.check_keyword("case")
                || self.check_keyword("default")
            {
                break;
            }
            if self.is_eof() {
                return Err(self.error_current("expected '}' after switch body"));
            }

            let starts_class_declaration = self.check_keyword("class");
            let statement = self.parse_statement()?;
            let needs_separator = !matches!(
                statement,
                Stmt::Block(_)
                    | Stmt::Empty
                    | Stmt::FunctionDeclaration(_)
                    | Stmt::If { .. }
                    | Stmt::While { .. }
                    | Stmt::DoWhile { .. }
                    | Stmt::For { .. }
                    | Stmt::Switch { .. }
                    | Stmt::Labeled { .. }
                    | Stmt::Try { .. }
            ) && !starts_class_declaration;
            statements.push(statement);

            if self.matches(&TokenKind::Semicolon) {
                continue;
            }
            if self.check(&TokenKind::RBrace)
                || self.check_keyword("case")
                || self.check_keyword("default")
            {
                continue;
            }
            if self.has_line_terminator_between_prev_and_current() {
                continue;
            }
            if needs_separator {
                return Err(self.error_current("expected ';' between statements"));
            }
        }
        Ok(statements)
    }

    fn parse_try_statement(&mut self) -> Result<Stmt, ParseError> {
        let try_block =
            self.parse_block_body("expected '{' after 'try'", "expected '}' after try block")?;

        let mut catch_param = None;
        let mut catch_block = None;
        let mut catch_parameter_effects = Vec::new();
        if self.matches_keyword("catch") {
            if self.matches(&TokenKind::LParen) {
                if self.check_identifier() {
                    catch_param = Some(Identifier(
                        self.expect_binding_identifier("expected catch binding identifier")?,
                    ));
                } else if self.matches(&TokenKind::LBracket) {
                    let temp_name = self.next_catch_temp_identifier();
                    let elements =
                        self.parse_for_head_array_pattern_after_lbracket(BindingKind::Let)?;
                    catch_parameter_effects.extend(Self::lower_catch_array_pattern_bindings(
                        &temp_name, &elements,
                    ));
                    catch_param = Some(temp_name);
                } else if self.check(&TokenKind::LBrace) {
                    let temp_name = self.next_catch_temp_identifier();
                    let effects = self.parse_object_parameter_pattern_effects(&temp_name)?;
                    catch_parameter_effects
                        .extend(Self::lower_catch_object_pattern_bindings(effects));
                    catch_param = Some(temp_name);
                } else {
                    return Err(self.error_current(
                        "expected catch binding identifier, array pattern, or object pattern",
                    ));
                }
                self.expect(TokenKind::RParen, "expected ')' after catch binding")?;
            }
            let mut parsed_catch_block = self.parse_block_body(
                "expected '{' before catch block",
                "expected '}' after catch block",
            )?;
            if !catch_parameter_effects.is_empty() {
                catch_parameter_effects.append(&mut parsed_catch_block);
                parsed_catch_block = catch_parameter_effects;
            }
            catch_block = Some(parsed_catch_block);
        }

        let finally_block = if self.matches_keyword("finally") {
            Some(self.parse_block_body(
                "expected '{' before finally block",
                "expected '}' after finally block",
            )?)
        } else {
            None
        };

        if catch_block.is_none() && finally_block.is_none() {
            return Err(self.error_current("try requires catch or finally"));
        }

        Ok(Stmt::Try {
            try_block,
            catch_param,
            catch_block,
            finally_block,
        })
    }

    fn lower_catch_array_pattern_bindings(
        temp_name: &Identifier,
        elements: &[ForHeadArrayPatternElement],
    ) -> Vec<Stmt> {
        let temp_expr = Expr::Identifier(temp_name.clone());
        elements
            .iter()
            .map(|element| {
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: element.name.clone(),
                    initializer: Some(Self::array_pattern_element_value_expr(&temp_expr, element)),
                })
            })
            .collect()
    }

    fn lower_catch_object_pattern_bindings(effects: Vec<Stmt>) -> Vec<Stmt> {
        effects
            .into_iter()
            .map(|statement| match statement {
                Stmt::Expression(Expr::Assign { target, value }) => {
                    Stmt::VariableDeclaration(VariableDeclaration {
                        kind: BindingKind::Let,
                        name: target,
                        initializer: Some(*value),
                    })
                }
                other => other,
            })
            .collect()
    }

    fn parse_throw_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.has_line_terminator_between_prev_and_current() {
            return Err(ParseError {
                message: "throw requires expression".to_string(),
                position: self.previous_position(),
            });
        }
        let has_expr = !matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof) | None
        );
        if !has_expr {
            return Err(ParseError {
                message: "throw requires expression".to_string(),
                position: self.previous_position(),
            });
        }
        let expr = self.parse_expression_with_commas()?;
        Ok(Stmt::Throw(expr))
    }

    fn parse_labeled_statement(&mut self) -> Result<Stmt, ParseError> {
        let label = Identifier(self.expect_binding_identifier("expected label identifier")?);
        if self
            .label_stack
            .iter()
            .any(|candidate| candidate == &label.0)
        {
            return Err(ParseError {
                message: format!("duplicate label: {}", label.0),
                position: self.previous_position(),
            });
        }
        self.expect(TokenKind::Colon, "expected ':' after label")?;
        self.label_stack.push(label.0.clone());
        let body = self.parse_embedded_statement(false);
        self.label_stack.pop();
        let body = body?;
        Ok(Stmt::Labeled {
            label,
            body: Box::new(body),
        })
    }

    fn parse_break_statement(&mut self) -> Result<Stmt, ParseError> {
        if !self.has_line_terminator_between_prev_and_current() && self.check_identifier() {
            let label = Identifier(
                self.expect_binding_identifier("expected label identifier after 'break'")?,
            );
            if !self
                .label_stack
                .iter()
                .any(|candidate| candidate == &label.0)
            {
                return Err(ParseError {
                    message: format!("undefined label: {}", label.0),
                    position: self.previous_position(),
                });
            }
            return Ok(Stmt::BreakLabel(label));
        }
        if self.breakable_depth == 0 {
            return Err(ParseError {
                message: "break outside loop or switch".to_string(),
                position: self.previous_position(),
            });
        }
        Ok(Stmt::Break)
    }

    fn parse_continue_statement(&mut self) -> Result<Stmt, ParseError> {
        if !self.has_line_terminator_between_prev_and_current() && self.check_identifier() {
            let label = Identifier(
                self.expect_binding_identifier("expected label identifier after 'continue'")?,
            );
            if !self
                .label_stack
                .iter()
                .any(|candidate| candidate == &label.0)
            {
                return Err(ParseError {
                    message: format!("undefined label: {}", label.0),
                    position: self.previous_position(),
                });
            }
            return Ok(Stmt::ContinueLabel(label));
        }
        if self.loop_depth == 0 {
            return Err(ParseError {
                message: "continue outside loop".to_string(),
                position: self.previous_position(),
            });
        }
        Ok(Stmt::Continue)
    }

    fn parse_embedded_statement(
        &mut self,
        allow_else_terminator: bool,
    ) -> Result<Stmt, ParseError> {
        if self.check_keyword("class") {
            return Err(self.error_current("class declaration not allowed in statement position"));
        }
        let mut statement = if self.check_keyword("let") && !self.check_next(&TokenKind::Colon) {
            Stmt::Expression(self.parse_expression_with_commas()?)
        } else {
            self.parse_statement()?
        };
        if matches!(statement, Stmt::FunctionDeclaration(_)) {
            // Annex B declaration forms in embedded statement positions should
            // still execute with block-scoped function bindings.
            statement = Stmt::Block(vec![statement]);
        }
        let needs_separator = !matches!(
            statement,
            Stmt::Block(_)
                | Stmt::Empty
                | Stmt::If { .. }
                | Stmt::While { .. }
                | Stmt::With { .. }
                | Stmt::DoWhile { .. }
                | Stmt::For { .. }
                | Stmt::Switch { .. }
                | Stmt::Labeled { .. }
                | Stmt::Try { .. }
        );
        if needs_separator && self.matches(&TokenKind::Semicolon) {
            return Ok(statement);
        }

        let can_end_without_separator = self.is_eof()
            || self.check(&TokenKind::RBrace)
            || (allow_else_terminator && self.check_keyword("else") && !needs_separator)
            || self.has_line_terminator_between_prev_and_current();
        if can_end_without_separator {
            return Ok(statement);
        }

        if needs_separator {
            return Err(self.error_current("expected ';' between statements"));
        }

        Ok(statement)
    }

    fn parse_return_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.function_depth == 0 {
            return Err(ParseError {
                message: "return outside function".to_string(),
                position: self.previous_position(),
            });
        }

        if self.has_line_terminator_between_prev_and_current() {
            return Ok(Stmt::Return(None));
        }
        let has_expr = !matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof) | None
        );
        if has_expr {
            let expr = self.parse_expression_with_commas()?;
            Ok(Stmt::Return(Some(expr)))
        } else {
            Ok(Stmt::Return(None))
        }
    }

    fn parse_variable_declaration(&mut self, kind: BindingKind) -> Result<Stmt, ParseError> {
        let mut declarations = Vec::new();
        loop {
            if self.matches(&TokenKind::LBracket) {
                let source_name = self.next_for_in_temp_identifier("decl_array");
                let source_expr = Expr::Identifier(source_name.clone());
                let elements = self.parse_for_head_array_pattern_after_lbracket(kind)?;
                self.expect(TokenKind::Equal, "expected '=' after array binding pattern")?;
                let initializer = self.parse_expression_inner()?;
                declarations.push(VariableDeclaration {
                    kind,
                    name: source_name,
                    initializer: Some(initializer),
                });
                for element in elements {
                    let value = Self::array_pattern_element_value_expr(&source_expr, &element);
                    declarations.push(VariableDeclaration {
                        kind,
                        name: element.name,
                        initializer: Some(value),
                    });
                }
            } else if self.check(&TokenKind::LBrace) {
                let source_name = self.next_for_in_temp_identifier("decl_object");
                let effects = self.parse_object_parameter_pattern_effects(&source_name)?;
                self.expect(
                    TokenKind::Equal,
                    "expected '=' after object binding pattern",
                )?;
                let initializer = self.parse_expression_inner()?;
                declarations.push(VariableDeclaration {
                    kind,
                    name: source_name,
                    initializer: Some(initializer),
                });
                declarations
                    .extend(self.lower_object_pattern_variable_declarations(kind, effects)?);
            } else {
                let name = if kind == BindingKind::Var {
                    self.expect_var_binding_identifier("expected binding name")?
                } else {
                    self.expect_binding_identifier("expected binding name")?
                };
                let initializer = if self.matches(&TokenKind::Equal) {
                    Some(self.parse_expression_inner()?)
                } else {
                    None
                };

                if kind == BindingKind::Const
                    && initializer.is_none()
                    && !(self.check_keyword("in") || self.check_keyword("of"))
                {
                    return Err(ParseError {
                        message: "const declaration requires an initializer".to_string(),
                        position: self.current_position(),
                    });
                }

                declarations.push(VariableDeclaration {
                    kind,
                    name: Identifier(name),
                    initializer,
                });
            }

            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }

        if declarations.len() == 1 {
            Ok(Stmt::VariableDeclaration(
                declarations
                    .into_iter()
                    .next()
                    .expect("declaration should exist"),
            ))
        } else {
            Ok(Stmt::VariableDeclarations(declarations))
        }
    }

    fn lower_object_pattern_variable_declarations(
        &mut self,
        kind: BindingKind,
        effects: Vec<Stmt>,
    ) -> Result<Vec<VariableDeclaration>, ParseError> {
        let mut declarations = Vec::new();
        for statement in effects {
            match statement {
                Stmt::Expression(Expr::Assign { target, value }) => {
                    declarations.push(VariableDeclaration {
                        kind,
                        name: target,
                        initializer: Some(*value),
                    });
                }
                Stmt::Expression(expr) => {
                    let effect_name = self.next_for_in_temp_identifier("decl_object_effect");
                    declarations.push(VariableDeclaration {
                        kind,
                        name: effect_name,
                        initializer: Some(expr),
                    });
                }
                _ => return Err(self.error_current("unsupported object binding pattern")),
            }
        }
        Ok(declarations)
    }

    fn parse_expression_inner(&mut self) -> Result<Expr, ParseError> {
        const MAX_EXPRESSION_DEPTH: usize = 40;
        self.expression_depth += 1;
        if self.expression_depth > MAX_EXPRESSION_DEPTH {
            self.expression_depth = self.expression_depth.saturating_sub(1);
            return Err(ParseError {
                message: "expression nesting too deep".to_string(),
                position: self.current_position(),
            });
        }

        let result = self.parse_assignment();
        self.expression_depth = self.expression_depth.saturating_sub(1);
        result
    }

    fn parse_expression_with_commas(&mut self) -> Result<Expr, ParseError> {
        let mut expressions = vec![self.parse_assignment()?];
        while self.matches(&TokenKind::Comma) {
            expressions.push(self.parse_assignment()?);
        }
        if expressions.len() == 1 {
            Ok(expressions.pop().expect("single expression should exist"))
        } else {
            Ok(Expr::Sequence(expressions))
        }
    }

    fn parse_expression_no_in(&mut self) -> Result<Expr, ParseError> {
        let saved_allow_in = self.allow_in;
        self.allow_in = false;
        let result = self.parse_expression_with_commas();
        self.allow_in = saved_allow_in;
        result
    }

    fn parse_expression_with_in(&mut self) -> Result<Expr, ParseError> {
        let saved_allow_in = self.allow_in;
        self.allow_in = true;
        let result = self.parse_expression_inner();
        self.allow_in = saved_allow_in;
        result
    }

    fn number_property_key(number: f64) -> String {
        if number.is_finite() && number.fract() == 0.0 {
            format!("{number:.0}")
        } else {
            number.to_string()
        }
    }

    fn parse_assignment(&mut self) -> Result<Expr, ParseError> {
        if let Some(arrow_function) = self.try_parse_arrow_function()? {
            return Ok(arrow_function);
        }

        let left = self.parse_conditional()?;
        let (assignment_kind, assignment_position) = if self.matches(&TokenKind::Equal) {
            (AssignmentKind::Simple, self.previous_position())
        } else if self.matches(&TokenKind::PlusEqual) {
            (
                AssignmentKind::Compound(BinaryOp::Add),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::MinusEqual) {
            (
                AssignmentKind::Compound(BinaryOp::Sub),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::StarEqual) {
            (
                AssignmentKind::Compound(BinaryOp::Mul),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::SlashEqual) {
            (
                AssignmentKind::Compound(BinaryOp::Div),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::PercentEqual) {
            (
                AssignmentKind::Compound(BinaryOp::Mod),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::LessLessEqual) {
            (
                AssignmentKind::Compound(BinaryOp::ShiftLeft),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::GreaterGreaterEqual) {
            (
                AssignmentKind::Compound(BinaryOp::ShiftRight),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::GreaterGreaterGreaterEqual) {
            (
                AssignmentKind::Compound(BinaryOp::UnsignedShiftRight),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::AmpEqual) {
            (
                AssignmentKind::Compound(BinaryOp::BitAnd),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::PipeEqual) {
            (
                AssignmentKind::Compound(BinaryOp::BitOr),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::CaretEqual) {
            (
                AssignmentKind::Compound(BinaryOp::BitXor),
                self.previous_position(),
            )
        } else if self.matches(&TokenKind::AndAndEqual) {
            (AssignmentKind::LogicalAnd, self.previous_position())
        } else if self.matches(&TokenKind::OrOrEqual) {
            (AssignmentKind::LogicalOr, self.previous_position())
        } else if self.matches(&TokenKind::QuestionQuestionEqual) {
            (AssignmentKind::Nullish, self.previous_position())
        } else {
            return Ok(left);
        };

        let right = self.parse_assignment()?;
        self.rewrite_assignment_target(left, right, assignment_kind, assignment_position)
    }

    fn rewrite_assignment_target(
        &mut self,
        left: Expr,
        right: Expr,
        assignment_kind: AssignmentKind,
        assignment_position: usize,
    ) -> Result<Expr, ParseError> {
        match left {
            Expr::Identifier(target) => {
                let read = Expr::Identifier(target.clone());
                let assign = Expr::Assign {
                    target: target.clone(),
                    value: Box::new(right),
                };
                match assignment_kind {
                    AssignmentKind::LogicalAnd
                    | AssignmentKind::LogicalOr
                    | AssignmentKind::Nullish => {
                        Ok(self.lower_short_circuit_assignment(read, assign, assignment_kind))
                    }
                    _ => {
                        let value = self.wrap_assignment_value(read, assign, assignment_kind);
                        if let Expr::Assign { value, .. } = value {
                            Ok(Expr::Assign { target, value })
                        } else {
                            unreachable!("assignment rewrite should preserve identifier target")
                        }
                    }
                }
            }
            Expr::Member { object, property } => {
                let read = Expr::Member {
                    object: object.clone(),
                    property: property.clone(),
                };
                let assign = Expr::AssignMember {
                    object: object.clone(),
                    property: property.clone(),
                    value: Box::new(right),
                };
                match assignment_kind {
                    AssignmentKind::LogicalAnd
                    | AssignmentKind::LogicalOr
                    | AssignmentKind::Nullish => {
                        Ok(self.lower_short_circuit_assignment(read, assign, assignment_kind))
                    }
                    _ => {
                        let value = self.wrap_assignment_value(read, assign, assignment_kind);
                        if let Expr::AssignMember { value, .. } = value {
                            Ok(Expr::AssignMember {
                                object,
                                property,
                                value,
                            })
                        } else {
                            unreachable!("assignment rewrite should preserve member target")
                        }
                    }
                }
            }
            Expr::MemberComputed { object, property } => {
                let read = Expr::MemberComputed {
                    object: object.clone(),
                    property: property.clone(),
                };
                let assign = Expr::AssignMemberComputed {
                    object: object.clone(),
                    property: property.clone(),
                    value: Box::new(right),
                };
                match assignment_kind {
                    AssignmentKind::LogicalAnd
                    | AssignmentKind::LogicalOr
                    | AssignmentKind::Nullish => {
                        Ok(self.lower_short_circuit_assignment(read, assign, assignment_kind))
                    }
                    _ => {
                        let value = self.wrap_assignment_value(read, assign, assignment_kind);
                        if let Expr::AssignMemberComputed { value, .. } = value {
                            Ok(Expr::AssignMemberComputed {
                                object,
                                property,
                                value,
                            })
                        } else {
                            unreachable!(
                                "assignment rewrite should preserve computed member target"
                            )
                        }
                    }
                }
            }
            Expr::ArrayLiteral(elements) => {
                if assignment_kind != AssignmentKind::Simple {
                    return Err(ParseError {
                        message: "invalid assignment target".to_string(),
                        position: assignment_position,
                    });
                }
                self.lower_array_assignment_pattern(elements, right, assignment_position)
            }
            Expr::ObjectLiteral(properties) => {
                if assignment_kind != AssignmentKind::Simple {
                    return Err(ParseError {
                        message: "invalid assignment target".to_string(),
                        position: assignment_position,
                    });
                }
                self.lower_object_assignment_pattern(properties, right, assignment_position)
            }
            other if Self::is_annex_b_call_assignment_target(&other) => {
                let _ = right;
                let _ = assignment_kind;
                Ok(Expr::AnnexBCallAssignmentTarget {
                    target: Box::new(other),
                })
            }
            _ => Err(ParseError {
                message: "invalid assignment target".to_string(),
                position: assignment_position,
            }),
        }
    }

    fn wrap_assignment_value(
        &self,
        read: Expr,
        assign_expr: Expr,
        assignment_kind: AssignmentKind,
    ) -> Expr {
        match assignment_kind {
            AssignmentKind::Simple => assign_expr,
            AssignmentKind::Compound(binary_op) => match assign_expr {
                Expr::Assign { target, value } => Expr::Assign {
                    target,
                    value: Box::new(Expr::Binary {
                        op: binary_op,
                        left: Box::new(read),
                        right: value,
                    }),
                },
                Expr::AssignMember {
                    object,
                    property,
                    value,
                } => Expr::AssignMember {
                    object,
                    property,
                    value: Box::new(Expr::Binary {
                        op: binary_op,
                        left: Box::new(read),
                        right: value,
                    }),
                },
                Expr::AssignMemberComputed {
                    object,
                    property,
                    value,
                } => Expr::AssignMemberComputed {
                    object,
                    property,
                    value: Box::new(Expr::Binary {
                        op: binary_op,
                        left: Box::new(read),
                        right: value,
                    }),
                },
                _ => unreachable!("assignment rewrite should produce assignment expression"),
            },
            AssignmentKind::LogicalAnd | AssignmentKind::LogicalOr | AssignmentKind::Nullish => {
                unreachable!("short-circuit assignment is rewritten separately")
            }
        }
    }

    fn lower_short_circuit_assignment(
        &mut self,
        current_value: Expr,
        assign_expr: Expr,
        assignment_kind: AssignmentKind,
    ) -> Expr {
        let current_name = self.next_for_in_temp_identifier("logical_assign_current");
        let current_ref = Expr::Identifier(current_name.clone());
        let condition = match assignment_kind {
            AssignmentKind::LogicalAnd | AssignmentKind::LogicalOr => current_ref.clone(),
            AssignmentKind::Nullish => self.build_nullish_check_expression(current_ref.clone()),
            AssignmentKind::Simple | AssignmentKind::Compound(_) => {
                unreachable!("expected short-circuit assignment kind")
            }
        };
        let (consequent, alternate) = match assignment_kind {
            AssignmentKind::LogicalAnd => (assign_expr, current_ref),
            AssignmentKind::LogicalOr => (current_ref, assign_expr),
            AssignmentKind::Nullish => (assign_expr, current_ref),
            AssignmentKind::Simple | AssignmentKind::Compound(_) => {
                unreachable!("expected short-circuit assignment kind")
            }
        };
        Expr::Call {
            callee: Box::new(Expr::Function {
                name: None,
                params: vec![current_name],
                body: vec![Stmt::Return(Some(Expr::Conditional {
                    condition: Box::new(condition),
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                }))],
            }),
            arguments: vec![current_value],
        }
    }

    fn lower_object_assignment_pattern(
        &mut self,
        properties: Vec<ObjectProperty>,
        right: Expr,
        assignment_position: usize,
    ) -> Result<Expr, ParseError> {
        let source_name = self.next_for_in_temp_identifier("obj_assign_source");
        let mut sequence = vec![Expr::Assign {
            target: source_name.clone(),
            value: Box::new(right),
        }];

        for property in properties {
            let key_expr = match property.key {
                ObjectPropertyKey::Static(name) => Expr::String(StringLiteral {
                    value: name,
                    has_escape: false,
                }),
                ObjectPropertyKey::ProtoSetter => Expr::String(StringLiteral {
                    value: "__proto__".to_string(),
                    has_escape: false,
                }),
                ObjectPropertyKey::Computed(expr) => *expr,
                ObjectPropertyKey::AccessorGet(_)
                | ObjectPropertyKey::AccessorSet(_)
                | ObjectPropertyKey::AccessorGetComputed(_)
                | ObjectPropertyKey::AccessorSetComputed(_) => {
                    return Err(ParseError {
                        message: "invalid assignment target".to_string(),
                        position: assignment_position,
                    });
                }
            };
            let Some((target_expr, default_initializer)) =
                self.extract_assignment_pattern_target(property.value, true, assignment_position)?
            else {
                return Err(ParseError {
                    message: "invalid assignment target".to_string(),
                    position: assignment_position,
                });
            };
            let read = Expr::MemberComputed {
                object: Box::new(Expr::Identifier(source_name.clone())),
                property: Box::new(key_expr),
            };
            let value = self.wrap_default_assignment_value(read, default_initializer);
            sequence.push(self.build_simple_assignment_target_expr(
                target_expr,
                value,
                assignment_position,
            )?);
        }

        sequence.push(Expr::Identifier(source_name));
        Ok(Expr::Sequence(sequence))
    }

    fn lower_array_assignment_pattern(
        &mut self,
        elements: Vec<Expr>,
        right: Expr,
        assignment_position: usize,
    ) -> Result<Expr, ParseError> {
        let source_name = self.next_for_in_temp_identifier("array_assign_source");
        let record_name = self.next_for_in_temp_identifier("array_assign_record");
        let done_name = self.next_for_in_temp_identifier("array_assign_done");
        let close_name = self.next_for_in_temp_identifier("array_assign_close");
        let caught_error_name = self.next_for_in_temp_identifier("array_assign_error");

        let mut try_block = Vec::new();
        for (index, element) in elements.into_iter().enumerate() {
            let category = format!("array_assign_current_{index}");
            let current_name = self.next_for_in_temp_identifier(&category);
            try_block.push(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: current_name.clone(),
                initializer: Some(Expr::Unary {
                    op: UnaryOp::Void,
                    expr: Box::new(Expr::Number(0.0)),
                }),
            }));

            let step_name = self.next_for_in_temp_identifier("array_assign_step");
            try_block.push(Stmt::If {
                condition: Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(Expr::Identifier(done_name.clone())),
                },
                consequent: Box::new(Stmt::Block(vec![
                    Stmt::VariableDeclaration(VariableDeclaration {
                        kind: BindingKind::Let,
                        name: step_name.clone(),
                        initializer: Some(Expr::Call {
                            callee: Box::new(Expr::Member {
                                object: Box::new(Expr::Identifier(Identifier(
                                    "Object".to_string(),
                                ))),
                                property: "__forOfStep".to_string(),
                            }),
                            arguments: vec![Expr::Identifier(record_name.clone())],
                        }),
                    }),
                    Stmt::If {
                        condition: Expr::Member {
                            object: Box::new(Expr::Identifier(step_name.clone())),
                            property: "done".to_string(),
                        },
                        consequent: Box::new(Stmt::Expression(Expr::Assign {
                            target: done_name.clone(),
                            value: Box::new(Expr::Bool(true)),
                        })),
                        alternate: Some(Box::new(Stmt::Expression(Expr::Assign {
                            target: current_name.clone(),
                            value: Box::new(Expr::Member {
                                object: Box::new(Expr::Identifier(step_name)),
                                property: "value".to_string(),
                            }),
                        }))),
                    },
                ])),
                alternate: None,
            });

            if let Some((target_expr, default_initializer)) =
                self.extract_assignment_pattern_target(element, false, assignment_position)?
            {
                let value = self.wrap_default_assignment_value(
                    Expr::Identifier(current_name),
                    default_initializer,
                );
                let assignment = self.build_simple_assignment_target_expr(
                    target_expr,
                    value,
                    assignment_position,
                )?;
                try_block.push(Stmt::Expression(assignment));
            }
        }
        try_block.push(Stmt::Expression(Expr::Assign {
            target: done_name.clone(),
            value: Box::new(Expr::Bool(true)),
        }));

        let body = vec![
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: record_name.clone(),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                        property: "__forOfIterator".to_string(),
                    }),
                    arguments: vec![Expr::Identifier(source_name.clone())],
                }),
            }),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: done_name.clone(),
                initializer: Some(Expr::Bool(false)),
            }),
            Stmt::Try {
                try_block,
                catch_param: Some(caught_error_name.clone()),
                catch_block: Some(vec![
                    Stmt::If {
                        condition: Expr::Unary {
                            op: UnaryOp::Not,
                            expr: Box::new(Expr::Identifier(done_name)),
                        },
                        consequent: Box::new(Stmt::Try {
                            try_block: vec![Stmt::VariableDeclaration(VariableDeclaration {
                                kind: BindingKind::Let,
                                name: close_name,
                                initializer: Some(Expr::Call {
                                    callee: Box::new(Expr::Member {
                                        object: Box::new(Expr::Identifier(Identifier(
                                            "Object".to_string(),
                                        ))),
                                        property: "__forOfClose".to_string(),
                                    }),
                                    arguments: vec![Expr::Identifier(record_name)],
                                }),
                            })],
                            catch_param: None,
                            catch_block: Some(Vec::new()),
                            finally_block: None,
                        }),
                        alternate: None,
                    },
                    Stmt::Throw(Expr::Identifier(caught_error_name)),
                ]),
                finally_block: None,
            },
            Stmt::Return(Some(Expr::Identifier(source_name.clone()))),
        ];

        Ok(Expr::Call {
            callee: Box::new(Expr::Function {
                name: None,
                params: vec![source_name],
                body,
            }),
            arguments: vec![right],
        })
    }

    fn extract_assignment_pattern_target(
        &self,
        expr: Expr,
        disallow_elision: bool,
        assignment_position: usize,
    ) -> Result<Option<(Expr, Option<Expr>)>, ParseError> {
        match expr {
            Expr::Elision if !disallow_elision => Ok(None),
            Expr::Identifier(_) | Expr::Member { .. } | Expr::MemberComputed { .. } => {
                Ok(Some((expr, None)))
            }
            Expr::Assign { target, value } => Ok(Some((Expr::Identifier(target), Some(*value)))),
            Expr::AssignMember {
                object,
                property,
                value,
            } => Ok(Some((Expr::Member { object, property }, Some(*value)))),
            Expr::AssignMemberComputed {
                object,
                property,
                value,
            } => Ok(Some((
                Expr::MemberComputed { object, property },
                Some(*value),
            ))),
            _ => Err(ParseError {
                message: "invalid assignment target".to_string(),
                position: assignment_position,
            }),
        }
    }

    fn wrap_default_assignment_value(
        &self,
        current_value: Expr,
        default_initializer: Option<Expr>,
    ) -> Expr {
        let Some(default_initializer) = default_initializer else {
            return current_value;
        };
        Expr::Conditional {
            condition: Box::new(Expr::Binary {
                op: BinaryOp::StrictEqual,
                left: Box::new(current_value.clone()),
                right: Box::new(Expr::Unary {
                    op: UnaryOp::Void,
                    expr: Box::new(Expr::Number(0.0)),
                }),
            }),
            consequent: Box::new(default_initializer),
            alternate: Box::new(current_value),
        }
    }

    fn build_simple_assignment_target_expr(
        &self,
        target: Expr,
        value: Expr,
        assignment_position: usize,
    ) -> Result<Expr, ParseError> {
        match target {
            Expr::Identifier(target) => Ok(Expr::Assign {
                target,
                value: Box::new(value),
            }),
            Expr::Member { object, property } => Ok(Expr::AssignMember {
                object,
                property,
                value: Box::new(value),
            }),
            Expr::MemberComputed { object, property } => Ok(Expr::AssignMemberComputed {
                object,
                property,
                value: Box::new(value),
            }),
            _ => Err(ParseError {
                message: "invalid assignment target".to_string(),
                position: assignment_position,
            }),
        }
    }

    fn try_parse_arrow_function(&mut self) -> Result<Option<Expr>, ParseError> {
        // Deeply nested parenthesized expressions should trip the expression-depth guard
        // instead of recursing through arrow-function lookahead.
        if self.expression_depth >= 64 {
            return Ok(None);
        }

        let saved_pos = self.pos;
        let mut is_async = false;

        let (params, simple_parameters, default_initializers, pattern_effects) =
            if self.matches(&TokenKind::LParen) {
                let params = match self.parse_parameter_list() {
                    Ok(parsed) => parsed,
                    Err(_) => {
                        self.pos = saved_pos;
                        return Ok(None);
                    }
                };
                if !self.matches(&TokenKind::RParen) {
                    self.pos = saved_pos;
                    return Ok(None);
                }
                params
            } else if self.check_identifier()
                && self.check_next(&TokenKind::Equal)
                && self.check_nth(2, &TokenKind::Greater)
            {
                (
                    vec![Identifier(
                        self.expect_binding_identifier("expected parameter name")?,
                    )],
                    true,
                    Vec::new(),
                    Vec::new(),
                )
            } else if self.check_keyword("async")
                && self.check_next(&TokenKind::LParen)
                && !self.has_line_terminator_between_tokens(self.pos, self.pos + 1)
            {
                self.matches_keyword("async");
                self.expect(TokenKind::LParen, "expected '(' after async")?;
                let params = match self.parse_parameter_list() {
                    Ok(parsed) => parsed,
                    Err(_) => {
                        self.pos = saved_pos;
                        return Ok(None);
                    }
                };
                if !self.matches(&TokenKind::RParen) {
                    self.pos = saved_pos;
                    return Ok(None);
                }
                is_async = true;
                params
            } else if self.check_keyword("async")
                && self.check_nth(2, &TokenKind::Equal)
                && self.check_nth(3, &TokenKind::Greater)
                && !self.has_line_terminator_between_tokens(self.pos, self.pos + 1)
                && matches!(
                    self.tokens.get(self.pos + 1).map(|token| &token.kind),
                    Some(TokenKind::Identifier(_))
                )
            {
                self.matches_keyword("async");
                is_async = true;
                (
                    vec![Identifier(
                        self.expect_binding_identifier("expected parameter name")?,
                    )],
                    true,
                    Vec::new(),
                    Vec::new(),
                )
            } else {
                return Ok(None);
            };

        if !self.matches(&TokenKind::Equal) || !self.matches(&TokenKind::Greater) {
            self.pos = saved_pos;
            return Ok(None);
        }

        for Identifier(name) in &params {
            if is_forbidden_identifier_reference(name) {
                return Err(self.error_current("expected parameter name"));
            }
        }

        let mut body = if self.check(&TokenKind::LBrace) {
            self.parse_function_body_with_context(
                "expected '{' before function body",
                "expected '}' after function body",
                false,
                is_async,
                false,
            )?
        } else {
            let saved_async_function_depth = self.async_function_depth;
            if is_async {
                self.async_function_depth = saved_async_function_depth + 1;
            }
            let body_expr = self.parse_assignment();
            self.async_function_depth = saved_async_function_depth;
            vec![Stmt::Return(Some(body_expr?))]
        };
        self.prepend_parameter_initializers(&mut body, &default_initializers, &pattern_effects);
        if !simple_parameters {
            self.prepend_non_simple_params_marker(&mut body);
        }
        self.prepend_arrow_function_marker(&mut body);
        if is_async {
            self.insert_async_function_marker(&mut body);
        }

        Ok(Some(Expr::Function {
            name: None,
            params,
            body,
        }))
    }

    fn parse_logical_or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_logical_and()?;
        while self.matches(&TokenKind::OrOr) {
            let right = self.parse_logical_and()?;
            expr = Expr::Binary {
                op: BinaryOp::LogicalOr,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn nullish_undefined_expression() -> Expr {
        Expr::Unary {
            op: UnaryOp::Void,
            expr: Box::new(Expr::Number(0.0)),
        }
    }

    fn build_nullish_check_expression(&self, value: Expr) -> Expr {
        Expr::Binary {
            op: BinaryOp::LogicalOr,
            left: Box::new(Expr::Binary {
                op: BinaryOp::StrictEqual,
                left: Box::new(value.clone()),
                right: Box::new(Expr::Null),
            }),
            right: Box::new(Expr::Binary {
                op: BinaryOp::StrictEqual,
                left: Box::new(value),
                right: Box::new(Self::nullish_undefined_expression()),
            }),
        }
    }

    fn lower_nullish_coalesce_expression(&mut self, left: Expr, right: Expr) -> Expr {
        let current_name = self.next_for_in_temp_identifier("coalesce_value");
        let current_ref = Expr::Identifier(current_name.clone());
        Expr::Call {
            callee: Box::new(Expr::Function {
                name: None,
                params: vec![current_name],
                body: vec![Stmt::Return(Some(Expr::Conditional {
                    condition: Box::new(self.build_nullish_check_expression(current_ref.clone())),
                    consequent: Box::new(right),
                    alternate: Box::new(current_ref),
                }))],
            }),
            arguments: vec![left],
        }
    }

    fn parse_coalesce(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_logical_or()?;
        while self.matches(&TokenKind::QuestionQuestion) {
            let right = self.parse_logical_or()?;
            expr = self.lower_nullish_coalesce_expression(expr, right);
        }
        Ok(expr)
    }

    fn parse_conditional(&mut self) -> Result<Expr, ParseError> {
        let condition = self.parse_coalesce()?;
        if !self.matches(&TokenKind::Question) {
            return Ok(condition);
        }
        let saved_allow_in = self.allow_in;
        self.allow_in = true;
        let consequent = self.parse_assignment();
        self.allow_in = saved_allow_in;
        let consequent = consequent?;
        self.expect(TokenKind::Colon, "expected ':' in conditional expression")?;
        let alternate = self.parse_assignment()?;
        Ok(Expr::Conditional {
            condition: Box::new(condition),
            consequent: Box::new(consequent),
            alternate: Box::new(alternate),
        })
    }

    fn parse_logical_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_bitwise_or()?;
        while self.matches(&TokenKind::AndAnd) {
            let right = self.parse_bitwise_or()?;
            expr = Expr::Binary {
                op: BinaryOp::LogicalAnd,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_bitwise_xor()?;
        while self.matches(&TokenKind::Pipe) {
            let right = self.parse_bitwise_xor()?;
            expr = Expr::Binary {
                op: BinaryOp::BitOr,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_bitwise_and()?;
        while self.matches(&TokenKind::Caret) {
            let right = self.parse_bitwise_and()?;
            expr = Expr::Binary {
                op: BinaryOp::BitXor,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_comparison()?;
        while self.matches(&TokenKind::Amp) {
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                op: BinaryOp::BitAnd,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_relational()?;
        loop {
            let op = if self.matches(&TokenKind::EqualEqualEqual) {
                BinaryOp::StrictEqual
            } else if self.matches(&TokenKind::BangEqualEqual) {
                BinaryOp::StrictNotEqual
            } else if self.matches(&TokenKind::EqualEqual) {
                BinaryOp::Equal
            } else if self.matches(&TokenKind::BangEqual) {
                BinaryOp::NotEqual
            } else {
                break;
            };
            let right = self.parse_relational()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_relational(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_shift()?;
        loop {
            let op = if self.matches(&TokenKind::Less) {
                BinaryOp::Less
            } else if self.matches(&TokenKind::LessEqual) {
                BinaryOp::LessEqual
            } else if self.matches(&TokenKind::Greater) {
                BinaryOp::Greater
            } else if self.matches(&TokenKind::GreaterEqual) {
                BinaryOp::GreaterEqual
            } else if self.allow_in && self.matches_keyword("in") {
                BinaryOp::In
            } else if self.matches_keyword("instanceof") {
                BinaryOp::InstanceOf
            } else {
                break;
            };
            let right = self.parse_shift()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_shift(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_additive()?;
        loop {
            let op = if self.matches(&TokenKind::LessLess) {
                BinaryOp::ShiftLeft
            } else if self.matches(&TokenKind::GreaterGreater) {
                BinaryOp::ShiftRight
            } else if self.matches(&TokenKind::GreaterGreaterGreater) {
                BinaryOp::UnsignedShiftRight
            } else {
                break;
            };
            let right = self.parse_additive()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_multiplicative()?;
        loop {
            let op = if self.matches(&TokenKind::Plus) {
                BinaryOp::Add
            } else if self.matches(&TokenKind::Minus) {
                BinaryOp::Sub
            } else {
                break;
            };
            let right = self.parse_multiplicative()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                BinaryOp::Mul
            } else if self.matches(&TokenKind::Slash) {
                BinaryOp::Div
            } else if self.matches(&TokenKind::Percent) {
                BinaryOp::Mod
            } else {
                break;
            };
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.matches(&TokenKind::PlusPlus) {
            return self.parse_prefix_update_expression(true);
        }
        if self.matches(&TokenKind::MinusMinus) {
            return self.parse_prefix_update_expression(false);
        }
        if self.matches(&TokenKind::Plus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(expr),
            });
        }
        if self.matches(&TokenKind::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(expr),
            });
        }
        if self.matches(&TokenKind::Bang) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.matches(&TokenKind::Tilde) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(expr),
            });
        }
        if self.matches_keyword("typeof") {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Typeof,
                expr: Box::new(expr),
            });
        }
        if self.matches_keyword("void") {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Void,
                expr: Box::new(expr),
            });
        }
        if self.matches_keyword("delete") {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Delete,
                expr: Box::new(expr),
            });
        }
        if self.async_function_depth > 0 && self.matches_keyword("await") {
            return self.parse_unary();
        }
        if self.matches_keyword("new") {
            return self.parse_new_expression();
        }
        self.parse_postfix()
    }

    fn parse_prefix_update_expression(&mut self, increment: bool) -> Result<Expr, ParseError> {
        let target = self.parse_unary()?;
        self.rewrite_update_target(target, increment, true)
    }

    fn rewrite_update_target(
        &self,
        target: Expr,
        increment: bool,
        prefix: bool,
    ) -> Result<Expr, ParseError> {
        match target {
            Expr::Identifier(identifier) => Ok(Expr::Update {
                target: UpdateTarget::Identifier(identifier),
                increment,
                prefix,
            }),
            Expr::Member { object, property } => Ok(Expr::Update {
                target: UpdateTarget::Member { object, property },
                increment,
                prefix,
            }),
            Expr::MemberComputed { object, property } => Ok(Expr::Update {
                target: UpdateTarget::MemberComputed { object, property },
                increment,
                prefix,
            }),
            other if Self::is_annex_b_call_assignment_target(&other) => {
                Ok(Expr::AnnexBCallAssignmentTarget {
                    target: Box::new(other),
                })
            }
            _ => Err(ParseError {
                message: "invalid update target".to_string(),
                position: self.current_position(),
            }),
        }
    }

    fn parse_new_expression(&mut self) -> Result<Expr, ParseError> {
        // NewExpression is right-recursive: `new NewExpression`.
        // Match QuickJS behavior where `new` recurses into postfix/new parsing.
        let mut callee = if self.matches_keyword("new") {
            self.parse_new_expression()?
        } else {
            self.parse_primary()?
        };
        loop {
            if self.matches(&TokenKind::Dot) {
                let property = self.expect_identifier_name("expected property name after '.'")?;
                callee = Expr::Member {
                    object: Box::new(callee),
                    property,
                };
                continue;
            }
            if self.matches(&TokenKind::LBracket) {
                let property = self.parse_expression_inner()?;
                self.expect(TokenKind::RBracket, "expected ']' after computed property")?;
                callee = Expr::MemberComputed {
                    object: Box::new(callee),
                    property: Box::new(property),
                };
                continue;
            }
            if self.check_template_part() {
                callee = self.parse_tagged_template_call(callee)?;
                continue;
            }
            break;
        }

        let arguments = if self.matches(&TokenKind::LParen) {
            let args = self.parse_argument_list()?;
            self.expect(TokenKind::RParen, "expected ')' after arguments")?;
            args
        } else {
            Vec::new()
        };
        let mut expr = Expr::New {
            callee: Box::new(callee),
            arguments,
        };
        loop {
            if self.matches(&TokenKind::LParen) {
                let arguments = self.parse_argument_list()?;
                self.expect(TokenKind::RParen, "expected ')' after arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    arguments,
                };
                continue;
            }
            if self.matches(&TokenKind::Dot) {
                let property = self.expect_identifier_name("expected property name after '.'")?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                };
                continue;
            }
            if self.matches(&TokenKind::LBracket) {
                let property = self.parse_expression_inner()?;
                self.expect(TokenKind::RBracket, "expected ']' after computed property")?;
                expr = Expr::MemberComputed {
                    object: Box::new(expr),
                    property: Box::new(property),
                };
                continue;
            }
            if self.check_template_part() {
                expr = self.parse_tagged_template_call(expr)?;
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.matches(&TokenKind::LParen) {
                let arguments = self.parse_argument_list()?;
                self.expect(TokenKind::RParen, "expected ')' after arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    arguments,
                };
                continue;
            }
            if self.matches(&TokenKind::Dot) {
                let property = self.expect_identifier_name("expected property name after '.'")?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                };
                continue;
            }
            if self.matches(&TokenKind::LBracket) {
                let property = self.parse_expression_inner()?;
                self.expect(TokenKind::RBracket, "expected ']' after computed property")?;
                expr = Expr::MemberComputed {
                    object: Box::new(expr),
                    property: Box::new(property),
                };
                continue;
            }
            if self.check_template_part() {
                expr = self.parse_tagged_template_call(expr)?;
                continue;
            }
            break;
        }
        if self.check(&TokenKind::PlusPlus) {
            if self.has_line_terminator_between_prev_and_current() {
                return Ok(expr);
            }
            self.advance();
            return self.rewrite_update_target(expr, true, false);
        }
        if self.check(&TokenKind::MinusMinus) {
            if self.has_line_terminator_between_prev_and_current() {
                return Ok(expr);
            }
            self.advance();
            return self.rewrite_update_target(expr, false, false);
        }
        Ok(expr)
    }

    fn parse_argument_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            let is_spread = self.matches(&TokenKind::Ellipsis);
            let argument = self.parse_expression_inner()?;
            if is_spread {
                args.push(Expr::SpreadArgument(Box::new(argument)));
            } else {
                args.push(argument);
            }
            if self.matches(&TokenKind::Comma) {
                if self.check(&TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(args)
    }

    fn parse_parameter_list(&mut self) -> Result<ParsedParameterList, ParseError> {
        let mut params = Vec::new();
        let mut synthetic_index = 0usize;
        let mut simple_parameters = true;
        let mut default_initializers = Vec::new();
        let mut pattern_effects = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok((
                params,
                simple_parameters,
                default_initializers,
                pattern_effects,
            ));
        }
        loop {
            let is_rest = self.matches(&TokenKind::Ellipsis);
            if is_rest {
                simple_parameters = false;
            }
            let param_index = params.len();
            let name = if self.check_identifier() {
                Identifier(self.expect_binding_identifier("expected parameter name")?)
            } else {
                simple_parameters = false;
                let generated = format!("$param_{synthetic_index}");
                synthetic_index += 1;
                let generated_identifier = Identifier(generated);
                let effects = if self.check(&TokenKind::LBrace) {
                    self.parse_object_parameter_pattern_effects(&generated_identifier)?
                } else if self.check(&TokenKind::LBracket) {
                    self.parse_array_parameter_pattern_effects(&generated_identifier)?
                } else {
                    return Err(self.error_current("expected parameter name"));
                };
                pattern_effects.extend(effects);
                generated_identifier
            };
            if is_rest {
                pattern_effects.push(Stmt::Expression(Expr::String(StringLiteral {
                    value: format!("{REST_PARAM_MARKER_PREFIX}{param_index}"),
                    has_escape: false,
                })));
            }
            if self.matches(&TokenKind::Equal) {
                simple_parameters = false;
                let initializer = self.parse_expression_inner()?;
                default_initializers.push((name.clone(), initializer));
            }
            params.push(name);
            if is_rest {
                break;
            }
            if self.matches(&TokenKind::Comma) {
                if self.check(&TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        Ok((
            params,
            simple_parameters,
            default_initializers,
            pattern_effects,
        ))
    }

    fn parse_object_parameter_pattern_effects(
        &mut self,
        synthetic_param: &Identifier,
    ) -> Result<Vec<Stmt>, ParseError> {
        self.expect(
            TokenKind::LBrace,
            "expected '{' in object parameter pattern",
        )?;
        let mut effects = Vec::new();
        if self.check(&TokenKind::RBrace) {
            self.advance();
            return Ok(effects);
        }
        loop {
            if self.matches(&TokenKind::Ellipsis) {
                self.consume_parameter_pattern("expected rest parameter pattern")?;
                break;
            }
            if self.matches(&TokenKind::LBracket) {
                let key_expr = self.parse_expression_inner()?;
                self.expect(
                    TokenKind::RBracket,
                    "expected ']' after computed property key",
                )?;
                effects.push(Stmt::Expression(key_expr));

                if self.matches(&TokenKind::Colon) {
                    if self.check_identifier() {
                        let _ = self.expect_binding_identifier("expected parameter name")?;
                    } else {
                        self.consume_parameter_pattern("expected parameter name")?;
                    }
                }
                if self.matches(&TokenKind::Equal) {
                    let initializer = self.parse_expression_inner()?;
                    effects.push(Stmt::Expression(initializer));
                }
            } else {
                let property_name = if self.check_identifier() {
                    self.expect_identifier_name("expected property name in parameter pattern")?
                } else if let Some(Token {
                    kind: TokenKind::String(name),
                    ..
                }) = self.current()
                {
                    let key = name.clone();
                    self.advance();
                    key
                } else if let Some(Token {
                    kind: TokenKind::Number(number),
                    ..
                }) = self.current()
                {
                    let key = if number.is_finite() && number.fract() == 0.0 {
                        format!("{number:.0}")
                    } else {
                        number.to_string()
                    };
                    self.advance();
                    key
                } else {
                    self.consume_parameter_pattern("expected parameter name")?;
                    String::new()
                };

                let mut binding_name = None;
                if self.matches(&TokenKind::Colon) {
                    if self.check_identifier() {
                        binding_name = Some(Identifier(
                            self.expect_binding_identifier("expected parameter name")?,
                        ));
                    } else {
                        self.consume_parameter_pattern("expected parameter name")?;
                    }
                } else if !property_name.is_empty()
                    && self.identifier_text_can_be_binding_name(&property_name)
                {
                    binding_name = Some(Identifier(property_name.clone()));
                }

                let default_initializer = if self.matches(&TokenKind::Equal) {
                    Some(self.parse_expression_inner()?)
                } else {
                    None
                };

                if let Some(binding_name) = binding_name {
                    let extracted = Expr::Member {
                        object: Box::new(Expr::Identifier(synthetic_param.clone())),
                        property: property_name,
                    };
                    let value = if let Some(default_initializer) = default_initializer {
                        Expr::Conditional {
                            condition: Box::new(Expr::Binary {
                                op: BinaryOp::StrictEqual,
                                left: Box::new(extracted.clone()),
                                right: Box::new(Expr::Identifier(Identifier(
                                    "undefined".to_string(),
                                ))),
                            }),
                            consequent: Box::new(default_initializer),
                            alternate: Box::new(extracted),
                        }
                    } else {
                        extracted
                    };
                    effects.push(Stmt::Expression(Expr::Assign {
                        target: binding_name,
                        value: Box::new(value),
                    }));
                } else if let Some(default_initializer) = default_initializer {
                    effects.push(Stmt::Expression(default_initializer));
                }
            }
            if self.matches(&TokenKind::Comma) {
                if self.check(&TokenKind::RBrace) {
                    break;
                }
                continue;
            }
            break;
        }
        self.expect(
            TokenKind::RBrace,
            "expected '}' after object parameter pattern",
        )?;
        Ok(effects)
    }

    fn parse_array_parameter_pattern_effects(
        &mut self,
        synthetic_param: &Identifier,
    ) -> Result<Vec<Stmt>, ParseError> {
        self.expect(
            TokenKind::LBracket,
            "expected '[' in array parameter pattern",
        )?;
        let mut effects = Vec::new();
        let mut index = 0usize;
        if self.check(&TokenKind::RBracket) {
            self.advance();
            return Ok(effects);
        }

        loop {
            if self.matches(&TokenKind::Comma) {
                index += 1;
                if self.check(&TokenKind::RBracket) {
                    break;
                }
                continue;
            }

            let is_rest = self.matches(&TokenKind::Ellipsis);
            if self.check_identifier() {
                let _ = self.expect_binding_identifier("expected parameter name")?;
            } else {
                self.consume_parameter_pattern("expected parameter name")?;
            }

            if self.matches(&TokenKind::Equal) {
                let initializer = self.parse_expression_inner()?;
                if is_rest {
                    effects.push(Stmt::Expression(initializer));
                } else {
                    let condition = Expr::Binary {
                        op: BinaryOp::LogicalOr,
                        left: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(Expr::Identifier(synthetic_param.clone())),
                            right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                        }),
                        right: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(Expr::MemberComputed {
                                object: Box::new(Expr::Identifier(synthetic_param.clone())),
                                property: Box::new(Expr::Number(index as f64)),
                            }),
                            right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                        }),
                    };
                    effects.push(Stmt::If {
                        condition,
                        consequent: Box::new(Stmt::Expression(initializer)),
                        alternate: None,
                    });
                }
            }

            if is_rest {
                break;
            }
            index += 1;
            if self.matches(&TokenKind::Comma) {
                if self.check(&TokenKind::RBracket) {
                    break;
                }
                continue;
            }
            break;
        }

        self.expect(
            TokenKind::RBracket,
            "expected ']' after array parameter pattern",
        )?;
        Ok(effects)
    }

    fn consume_parameter_pattern(&mut self, error_message: &str) -> Result<(), ParseError> {
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut brace_depth = 0usize;
        let mut consumed_any = false;

        loop {
            let Some(token) = self.current().cloned() else {
                break;
            };
            match token.kind {
                TokenKind::Eof => break,
                TokenKind::Comma if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                    break;
                }
                TokenKind::RParen if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                    break;
                }
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => {
                    if paren_depth == 0 {
                        break;
                    }
                    paren_depth -= 1;
                }
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => {
                    if bracket_depth == 0 {
                        break;
                    }
                    bracket_depth -= 1;
                }
                TokenKind::LBrace => brace_depth += 1,
                TokenKind::RBrace => {
                    if brace_depth == 0 {
                        break;
                    }
                    brace_depth -= 1;
                }
                _ => {}
            }
            self.advance();
            consumed_any = true;
        }

        if !consumed_any {
            return Err(self.error_current(error_message));
        }
        Ok(())
    }

    fn parse_function_expression_after_keyword(
        &mut self,
        is_async: bool,
    ) -> Result<Expr, ParseError> {
        let is_generator = self.matches(&TokenKind::Star);
        let name = if self.check_identifier() {
            Some(Identifier(
                self.expect_binding_identifier("expected function name")?,
            ))
        } else {
            None
        };
        self.expect(TokenKind::LParen, "expected '(' after 'function'")?;
        let (params, simple_parameters, default_initializers, pattern_effects) =
            self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        let mut body = self.parse_function_body_with_context(
            "expected '{' before function body",
            "expected '}' after function body",
            false,
            is_async,
            is_generator,
        )?;
        self.prepend_parameter_initializers(&mut body, &default_initializers, &pattern_effects);
        if !simple_parameters {
            self.prepend_non_simple_params_marker(&mut body);
        }
        if is_async {
            self.insert_async_function_marker(&mut body);
        }
        if name.is_some() {
            self.insert_named_function_expression_marker(&mut body);
        }
        if is_generator {
            self.insert_generator_function_marker(&mut body);
        }

        Ok(Expr::Function { name, params, body })
    }

    fn parse_class_expression_after_keyword(&mut self) -> Result<Expr, ParseError> {
        let class_name = if self.check_identifier() && !self.check_keyword("extends") {
            Some(Identifier(
                self.expect_binding_identifier("expected class name")?,
            ))
        } else {
            None
        };
        let class_tail = self.parse_class_tail()?;
        Ok(self.lower_class_tail(class_tail, class_name))
    }

    fn parse_class_tail(&mut self) -> Result<ParsedClassTail, ParseError> {
        let checkpoint = self.pos;
        match self.parse_class_tail_detailed() {
            Ok(parsed) => Ok(parsed),
            Err(_) => {
                self.pos = checkpoint;
                if self.matches_keyword("extends") {
                    let _ = self.parse_expression_inner()?;
                }
                self.consume_balanced_brace_block(
                    "expected '{' before class body",
                    "expected '}' after class body",
                )?;
                Ok(ParsedClassTail::default())
            }
        }
    }

    fn parse_class_tail_detailed(&mut self) -> Result<ParsedClassTail, ParseError> {
        let mut parsed = ParsedClassTail::default();
        if self.matches_keyword("extends") {
            parsed.extends = Some(self.parse_expression_inner()?);
        }
        self.expect(TokenKind::LBrace, "expected '{' before class body")?;
        while !self.check(&TokenKind::RBrace) {
            if self.is_eof() {
                return Err(self.error_current("expected '}' after class body"));
            }
            if self.matches(&TokenKind::Semicolon) {
                continue;
            }

            let is_static = self.check_keyword("static") && !self.check_next(&TokenKind::LParen);
            if is_static {
                self.advance();
            }

            let kind = if self.check_keyword("get") && !self.check_next(&TokenKind::LParen) {
                self.advance();
                ClassElementKind::Getter
            } else if self.check_keyword("set") && !self.check_next(&TokenKind::LParen) {
                self.advance();
                ClassElementKind::Setter
            } else {
                ClassElementKind::Method
            };
            let is_generator_method = self.matches(&TokenKind::Star);

            let key = self.parse_class_method_name()?;
            self.expect(TokenKind::LParen, "expected '(' after method name")?;
            let (params, simple_parameters, default_initializers, pattern_effects) =
                self.parse_parameter_list()?;
            self.expect(TokenKind::RParen, "expected ')' after parameters")?;
            let body = self.parse_function_body_with_super_policy(
                "expected '{' before method body",
                "expected '}' after method body",
                true,
                is_generator_method,
            );
            let mut body = body?;
            if matches!(kind, ClassElementKind::Getter) && !params.is_empty() {
                return Err(self.error_current("getter must not have parameters"));
            }
            if matches!(kind, ClassElementKind::Setter) && params.len() != 1 {
                return Err(self.error_current("setter must have exactly one parameter"));
            }
            self.prepend_parameter_initializers(&mut body, &default_initializers, &pattern_effects);
            if !simple_parameters {
                self.prepend_non_simple_params_marker(&mut body);
            }
            if is_generator_method {
                self.insert_generator_function_marker(&mut body);
            }
            parsed.methods.push(ClassMethodDefinition {
                key,
                value: Expr::Function {
                    name: None,
                    params,
                    body,
                },
                is_static,
                kind,
            });
        }

        self.expect(TokenKind::RBrace, "expected '}' after class body")?;
        Ok(parsed)
    }

    fn parse_class_method_name(&mut self) -> Result<ClassMethodKey, ParseError> {
        let token = self.current().cloned().ok_or(ParseError {
            message: "expected method name in class body".to_string(),
            position: self.last_position(),
        })?;
        match token.kind {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(ClassMethodKey::Static(name))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(ClassMethodKey::Static(name))
            }
            TokenKind::Number(number) => {
                self.advance();
                let key = Self::number_property_key(number);
                Ok(ClassMethodKey::Static(key))
            }
            TokenKind::LBracket => {
                self.advance();
                let key = self.parse_expression_with_in()?;
                self.expect(
                    TokenKind::RBracket,
                    "expected ']' after computed method name",
                )?;
                Ok(ClassMethodKey::Computed(key))
            }
            _ => Err(ParseError {
                message: "expected method name in class body".to_string(),
                position: token.span.start,
            }),
        }
    }

    fn lower_class_tail(
        &mut self,
        class_tail: ParsedClassTail,
        class_name: Option<Identifier>,
    ) -> Expr {
        let class_temp = self.next_class_temp_name();
        let class_ident = Identifier(class_temp.clone());
        let ParsedClassTail { methods, extends } = class_tail;
        let has_extends = extends.is_some();
        let super_ident = Identifier("$__class_super".to_string());
        let mut constructor_value = Expr::Function {
            name: None,
            params: vec![],
            body: vec![],
        };
        let mut has_explicit_constructor = false;
        let mut lowered_methods = Vec::new();
        for method in methods {
            let is_constructor = !method.is_static
                && matches!(method.kind, ClassElementKind::Method)
                && matches!(&method.key, ClassMethodKey::Static(name) if name == "constructor");
            if is_constructor {
                constructor_value = self.lower_class_method_with_super_binding(
                    method.value,
                    false,
                    &class_ident,
                    true,
                    if has_extends {
                        Some(Expr::Identifier(super_ident.clone()))
                    } else {
                        None
                    },
                );
                has_explicit_constructor = true;
                continue;
            }
            lowered_methods.push(method);
        }

        if extends.is_some() && !has_explicit_constructor {
            constructor_value = self.lower_class_method_with_super_binding(
                Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![Stmt::Return(Some(Expr::Call {
                        callee: Box::new(Expr::Identifier(Identifier("super".to_string()))),
                        arguments: vec![Expr::SpreadArgument(Box::new(Expr::Identifier(
                            Identifier("arguments".to_string()),
                        )))],
                    }))],
                },
                false,
                &class_ident,
                true,
                Some(Expr::Identifier(super_ident.clone())),
            );
        }

        let mut body = vec![
            Stmt::Expression(Expr::String(StringLiteral {
                value: "use strict".to_string(),
                has_escape: false,
            })),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: class_ident.clone(),
                initializer: Some(constructor_value),
            }),
        ];

        let prototype_value = if let Some(extends_expr) = extends {
            body.push(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: super_ident.clone(),
                initializer: Some(extends_expr),
            }));
            body.push(Stmt::Expression(Expr::Conditional {
                condition: Box::new(Expr::Binary {
                    op: BinaryOp::StrictEqual,
                    left: Box::new(Expr::Identifier(super_ident.clone())),
                    right: Box::new(Expr::Null),
                }),
                consequent: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                alternate: Box::new(Expr::AssignMember {
                    object: Box::new(Expr::Identifier(super_ident.clone())),
                    property: CLASS_HERITAGE_RESTRICTED_MARKER.to_string(),
                    value: Box::new(Expr::Bool(true)),
                }),
            }));
            let super_parent_prototype = Expr::Conditional {
                condition: Box::new(Expr::Binary {
                    op: BinaryOp::StrictEqual,
                    left: Box::new(Expr::Identifier(super_ident.clone())),
                    right: Box::new(Expr::Null),
                }),
                consequent: Box::new(Expr::Null),
                alternate: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(super_ident.clone())),
                    property: "prototype".to_string(),
                }),
            };
            Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                    property: "create".to_string(),
                }),
                arguments: vec![super_parent_prototype],
            }
        } else {
            Expr::ObjectLiteral(vec![])
        };
        if let Some(class_name) = class_name {
            body.push(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Const,
                name: class_name,
                initializer: Some(Expr::Identifier(class_ident.clone())),
            }));
        }

        body.push(Stmt::Expression(Expr::AssignMember {
            object: Box::new(Expr::Identifier(class_ident.clone())),
            property: CLASS_CONSTRUCTOR_MARKER.to_string(),
            value: Box::new(Expr::Bool(true)),
        }));
        if has_extends {
            body.push(Stmt::Expression(Expr::AssignMember {
                object: Box::new(Expr::Identifier(class_ident.clone())),
                property: CLASS_DERIVED_CONSTRUCTOR_MARKER.to_string(),
                value: Box::new(Expr::Bool(true)),
            }));
            body.push(Stmt::Expression(Expr::AssignMember {
                object: Box::new(Expr::Identifier(class_ident.clone())),
                property: CLASS_CONSTRUCTOR_PARENT_MARKER.to_string(),
                value: Box::new(Expr::Conditional {
                    condition: Box::new(Expr::Binary {
                        op: BinaryOp::StrictEqual,
                        left: Box::new(Expr::Identifier(super_ident.clone())),
                        right: Box::new(Expr::Null),
                    }),
                    consequent: Box::new(Expr::Member {
                        object: Box::new(Expr::Identifier(Identifier("Function".to_string()))),
                        property: "prototype".to_string(),
                    }),
                    alternate: Box::new(Expr::Identifier(super_ident.clone())),
                }),
            }));
        }
        body.extend([
            Stmt::Expression(Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                    property: "defineProperty".to_string(),
                }),
                arguments: vec![
                    Expr::Identifier(class_ident.clone()),
                    Expr::String(StringLiteral {
                        value: "prototype".to_string(),
                        has_escape: false,
                    }),
                    Expr::ObjectLiteral(vec![
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("value".to_string()),
                            value: prototype_value,
                        },
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("writable".to_string()),
                            value: Expr::Bool(false),
                        },
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("enumerable".to_string()),
                            value: Expr::Bool(false),
                        },
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("configurable".to_string()),
                            value: Expr::Bool(false),
                        },
                    ]),
                ],
            }),
            Stmt::Expression(Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                    property: "defineProperty".to_string(),
                }),
                arguments: vec![
                    Expr::Member {
                        object: Box::new(Expr::Identifier(class_ident.clone())),
                        property: "prototype".to_string(),
                    },
                    Expr::String(StringLiteral {
                        value: "constructor".to_string(),
                        has_escape: false,
                    }),
                    Expr::ObjectLiteral(vec![
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("value".to_string()),
                            value: Expr::Identifier(class_ident.clone()),
                        },
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("writable".to_string()),
                            value: Expr::Bool(true),
                        },
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("enumerable".to_string()),
                            value: Expr::Bool(false),
                        },
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("configurable".to_string()),
                            value: Expr::Bool(true),
                        },
                    ]),
                ],
            }),
        ]);

        for method in lowered_methods {
            let ClassMethodDefinition {
                key,
                value,
                is_static,
                kind,
            } = method;
            if is_static && self.class_method_key_is_prototype(&key) {
                body.push(Stmt::Throw(Expr::String(StringLiteral {
                    value: "TypeError: static class member named prototype".to_string(),
                    has_escape: false,
                })));
                continue;
            }

            let method_super_override = if has_extends {
                if is_static {
                    Some(Expr::Identifier(super_ident.clone()))
                } else {
                    Some(Expr::Conditional {
                        condition: Box::new(Expr::Binary {
                            op: BinaryOp::StrictEqual,
                            left: Box::new(Expr::Identifier(super_ident.clone())),
                            right: Box::new(Expr::Null),
                        }),
                        consequent: Box::new(Expr::Null),
                        alternate: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(super_ident.clone())),
                            property: "prototype".to_string(),
                        }),
                    })
                }
            } else {
                None
            };
            let method_value = self.mark_class_method_non_constructible(
                self.lower_class_method_with_super_binding(
                    value,
                    is_static,
                    &class_ident,
                    false,
                    method_super_override,
                ),
            );
            let target = if is_static {
                Expr::Identifier(class_ident.clone())
            } else {
                Expr::Member {
                    object: Box::new(Expr::Identifier(class_ident.clone())),
                    property: "prototype".to_string(),
                }
            };
            match kind {
                ClassElementKind::Method => {
                    let key_expr = match key {
                        ClassMethodKey::Static(name) => Expr::String(StringLiteral {
                            value: name,
                            has_escape: false,
                        }),
                        ClassMethodKey::Computed(key) => key,
                    };
                    body.push(Stmt::Expression(Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                            property: "defineProperty".to_string(),
                        }),
                        arguments: vec![
                            target,
                            key_expr,
                            Expr::ObjectLiteral(vec![
                                ObjectProperty {
                                    key: ObjectPropertyKey::Static("value".to_string()),
                                    value: method_value,
                                },
                                ObjectProperty {
                                    key: ObjectPropertyKey::Static("writable".to_string()),
                                    value: Expr::Bool(true),
                                },
                                ObjectProperty {
                                    key: ObjectPropertyKey::Static("enumerable".to_string()),
                                    value: Expr::Bool(false),
                                },
                                ObjectProperty {
                                    key: ObjectPropertyKey::Static("configurable".to_string()),
                                    value: Expr::Bool(true),
                                },
                            ]),
                        ],
                    }));
                }
                ClassElementKind::Getter | ClassElementKind::Setter => {
                    let key_expr = match key {
                        ClassMethodKey::Static(name) => Expr::String(StringLiteral {
                            value: name,
                            has_escape: false,
                        }),
                        ClassMethodKey::Computed(key) => key,
                    };
                    let mut descriptor_properties = vec![
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("configurable".to_string()),
                            value: Expr::Bool(true),
                        },
                        ObjectProperty {
                            key: ObjectPropertyKey::Static("enumerable".to_string()),
                            value: Expr::Bool(false),
                        },
                    ];
                    let accessor_name = if matches!(kind, ClassElementKind::Getter) {
                        "get"
                    } else {
                        "set"
                    };
                    descriptor_properties.push(ObjectProperty {
                        key: ObjectPropertyKey::Static(accessor_name.to_string()),
                        value: method_value,
                    });
                    body.push(Stmt::Expression(Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                            property: "defineProperty".to_string(),
                        }),
                        arguments: vec![
                            target,
                            key_expr,
                            Expr::ObjectLiteral(descriptor_properties),
                        ],
                    }));
                }
            }
        }

        body.push(Stmt::Return(Some(Expr::Identifier(class_ident))));

        Expr::Call {
            callee: Box::new(Expr::Function {
                name: None,
                params: vec![],
                body,
            }),
            arguments: vec![],
        }
    }

    fn next_class_temp_name(&mut self) -> String {
        let name = format!("$__class_ctor_{}", self.class_temp_index);
        self.class_temp_index += 1;
        name
    }

    fn lower_class_method_with_super_binding(
        &self,
        value: Expr,
        is_static: bool,
        class_ident: &Identifier,
        constructor_super: bool,
        super_override: Option<Expr>,
    ) -> Expr {
        let Expr::Function { name, params, body } = value else {
            return value;
        };
        let rewrite_constructor_super_base = constructor_super && super_override.is_some();
        let super_value = if let Some(override_expr) = super_override {
            override_expr
        } else {
            let super_anchor = if is_static || constructor_super {
                Expr::Identifier(class_ident.clone())
            } else {
                Expr::Member {
                    object: Box::new(Expr::Identifier(class_ident.clone())),
                    property: "prototype".to_string(),
                }
            };
            Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                    property: "getPrototypeOf".to_string(),
                }),
                arguments: vec![super_anchor],
            }
        };
        let mut lowered_body = vec![
            Stmt::Expression(Expr::String(StringLiteral {
                value: "use strict".to_string(),
                has_escape: false,
            })),
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: Identifier("super".to_string()),
                initializer: Some(super_value),
            }),
        ];
        let lowered_user_body = if rewrite_constructor_super_base {
            lowered_body.push(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: Identifier(CLASS_CONSTRUCTOR_SUPER_BASE_BINDING.to_string()),
                initializer: Some(Expr::Call {
                    callee: Box::new(Expr::Function {
                        name: None,
                        params: vec![Identifier("$__super_ctor".to_string())],
                        body: vec![Stmt::Return(Some(Expr::Conditional {
                            condition: Box::new(Expr::Binary {
                                op: BinaryOp::StrictEqual,
                                left: Box::new(Expr::Identifier(Identifier(
                                    "$__super_ctor".to_string(),
                                ))),
                                right: Box::new(Expr::Null),
                            }),
                            consequent: Box::new(Expr::Null),
                            alternate: Box::new(Expr::Member {
                                object: Box::new(Expr::Identifier(Identifier(
                                    "$__super_ctor".to_string(),
                                ))),
                                property: "prototype".to_string(),
                            }),
                        }))],
                    }),
                    arguments: vec![Expr::Identifier(Identifier("super".to_string()))],
                }),
            }));
            body.into_iter()
                .map(Self::rewrite_constructor_super_property_stmt)
                .collect()
        } else {
            body
        };
        lowered_body.extend(lowered_user_body);
        Expr::Function {
            name,
            params,
            body: lowered_body,
        }
    }

    fn rewrite_constructor_super_property_stmt(stmt: Stmt) -> Stmt {
        match stmt {
            Stmt::Empty => Stmt::Empty,
            Stmt::VariableDeclaration(VariableDeclaration {
                kind,
                name,
                initializer,
            }) => Stmt::VariableDeclaration(VariableDeclaration {
                kind,
                name,
                initializer: initializer.map(Self::rewrite_constructor_super_property_expr),
            }),
            Stmt::VariableDeclarations(declarations) => Stmt::VariableDeclarations(
                declarations
                    .into_iter()
                    .map(
                        |VariableDeclaration {
                             kind,
                             name,
                             initializer,
                         }| VariableDeclaration {
                            kind,
                            name,
                            initializer: initializer
                                .map(Self::rewrite_constructor_super_property_expr),
                        },
                    )
                    .collect(),
            ),
            Stmt::FunctionDeclaration(FunctionDeclaration { name, params, body }) => {
                Stmt::FunctionDeclaration(FunctionDeclaration {
                    name,
                    params,
                    body: body
                        .into_iter()
                        .map(Self::rewrite_constructor_super_property_stmt)
                        .collect(),
                })
            }
            Stmt::Return(expr) => {
                Stmt::Return(expr.map(Self::rewrite_constructor_super_property_expr))
            }
            Stmt::Expression(expr) => {
                Stmt::Expression(Self::rewrite_constructor_super_property_expr(expr))
            }
            Stmt::Block(statements) => Stmt::Block(
                statements
                    .into_iter()
                    .map(Self::rewrite_constructor_super_property_stmt)
                    .collect(),
            ),
            Stmt::If {
                condition,
                consequent,
                alternate,
            } => Stmt::If {
                condition: Self::rewrite_constructor_super_property_expr(condition),
                consequent: Box::new(Self::rewrite_constructor_super_property_stmt(*consequent)),
                alternate: alternate
                    .map(|stmt| Box::new(Self::rewrite_constructor_super_property_stmt(*stmt))),
            },
            Stmt::While { condition, body } => Stmt::While {
                condition: Self::rewrite_constructor_super_property_expr(condition),
                body: Box::new(Self::rewrite_constructor_super_property_stmt(*body)),
            },
            Stmt::With { object, body } => Stmt::With {
                object: Self::rewrite_constructor_super_property_expr(object),
                body: Box::new(Self::rewrite_constructor_super_property_stmt(*body)),
            },
            Stmt::DoWhile { body, condition } => Stmt::DoWhile {
                body: Box::new(Self::rewrite_constructor_super_property_stmt(*body)),
                condition: Self::rewrite_constructor_super_property_expr(condition),
            },
            Stmt::For {
                initializer,
                condition,
                update,
                body,
            } => Stmt::For {
                initializer: initializer.map(|init| match init {
                    ForInitializer::VariableDeclaration(VariableDeclaration {
                        kind,
                        name,
                        initializer,
                    }) => ForInitializer::VariableDeclaration(VariableDeclaration {
                        kind,
                        name,
                        initializer: initializer.map(Self::rewrite_constructor_super_property_expr),
                    }),
                    ForInitializer::VariableDeclarations(declarations) => {
                        ForInitializer::VariableDeclarations(
                            declarations
                                .into_iter()
                                .map(
                                    |VariableDeclaration {
                                         kind,
                                         name,
                                         initializer,
                                     }| VariableDeclaration {
                                        kind,
                                        name,
                                        initializer: initializer
                                            .map(Self::rewrite_constructor_super_property_expr),
                                    },
                                )
                                .collect(),
                        )
                    }
                    ForInitializer::Expression(expr) => ForInitializer::Expression(
                        Self::rewrite_constructor_super_property_expr(expr),
                    ),
                }),
                condition: condition.map(Self::rewrite_constructor_super_property_expr),
                update: update.map(Self::rewrite_constructor_super_property_expr),
                body: Box::new(Self::rewrite_constructor_super_property_stmt(*body)),
            },
            Stmt::Switch {
                discriminant,
                cases,
            } => Stmt::Switch {
                discriminant: Self::rewrite_constructor_super_property_expr(discriminant),
                cases: cases
                    .into_iter()
                    .map(|SwitchCase { test, consequent }| SwitchCase {
                        test: test.map(Self::rewrite_constructor_super_property_expr),
                        consequent: consequent
                            .into_iter()
                            .map(Self::rewrite_constructor_super_property_stmt)
                            .collect(),
                    })
                    .collect(),
            },
            Stmt::Throw(expr) => Stmt::Throw(Self::rewrite_constructor_super_property_expr(expr)),
            Stmt::Try {
                try_block,
                catch_param,
                catch_block,
                finally_block,
            } => Stmt::Try {
                try_block: try_block
                    .into_iter()
                    .map(Self::rewrite_constructor_super_property_stmt)
                    .collect(),
                catch_param,
                catch_block: catch_block.map(|block| {
                    block
                        .into_iter()
                        .map(Self::rewrite_constructor_super_property_stmt)
                        .collect()
                }),
                finally_block: finally_block.map(|block| {
                    block
                        .into_iter()
                        .map(Self::rewrite_constructor_super_property_stmt)
                        .collect()
                }),
            },
            Stmt::Labeled { label, body } => Stmt::Labeled {
                label,
                body: Box::new(Self::rewrite_constructor_super_property_stmt(*body)),
            },
            Stmt::Break => Stmt::Break,
            Stmt::BreakLabel(label) => Stmt::BreakLabel(label),
            Stmt::Continue => Stmt::Continue,
            Stmt::ContinueLabel(label) => Stmt::ContinueLabel(label),
        }
    }

    fn rewrite_constructor_super_property_expr(expr: Expr) -> Expr {
        match expr {
            Expr::Number(value) => Expr::Number(value),
            Expr::Bool(value) => Expr::Bool(value),
            Expr::Null => Expr::Null,
            Expr::String(value) => Expr::String(value),
            Expr::RegexLiteral { pattern, flags } => Expr::RegexLiteral { pattern, flags },
            Expr::This => Expr::This,
            Expr::Identifier(identifier) => Expr::Identifier(identifier),
            Expr::Function { name, params, body } => Expr::Function {
                name,
                params,
                body: body
                    .into_iter()
                    .map(Self::rewrite_constructor_super_property_stmt)
                    .collect(),
            },
            Expr::ObjectLiteral(properties) => Expr::ObjectLiteral(
                properties
                    .into_iter()
                    .map(|ObjectProperty { key, value }| ObjectProperty {
                        key: match key {
                            ObjectPropertyKey::Static(name) => ObjectPropertyKey::Static(name),
                            ObjectPropertyKey::ProtoSetter => ObjectPropertyKey::ProtoSetter,
                            ObjectPropertyKey::Computed(key_expr) => ObjectPropertyKey::Computed(
                                Box::new(Self::rewrite_constructor_super_property_expr(*key_expr)),
                            ),
                            ObjectPropertyKey::AccessorGet(name) => {
                                ObjectPropertyKey::AccessorGet(name)
                            }
                            ObjectPropertyKey::AccessorSet(name) => {
                                ObjectPropertyKey::AccessorSet(name)
                            }
                            ObjectPropertyKey::AccessorGetComputed(key_expr) => {
                                ObjectPropertyKey::AccessorGetComputed(Box::new(
                                    Self::rewrite_constructor_super_property_expr(*key_expr),
                                ))
                            }
                            ObjectPropertyKey::AccessorSetComputed(key_expr) => {
                                ObjectPropertyKey::AccessorSetComputed(Box::new(
                                    Self::rewrite_constructor_super_property_expr(*key_expr),
                                ))
                            }
                        },
                        value: Self::rewrite_constructor_super_property_expr(value),
                    })
                    .collect(),
            ),
            Expr::ArrayLiteral(items) => Expr::ArrayLiteral(
                items
                    .into_iter()
                    .map(Self::rewrite_constructor_super_property_expr)
                    .collect(),
            ),
            Expr::Elision => Expr::Elision,
            Expr::Unary { op, expr } => Expr::Unary {
                op,
                expr: Box::new(Self::rewrite_constructor_super_property_expr(*expr)),
            },
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => Expr::Conditional {
                condition: Box::new(Self::rewrite_constructor_super_property_expr(*condition)),
                consequent: Box::new(Self::rewrite_constructor_super_property_expr(*consequent)),
                alternate: Box::new(Self::rewrite_constructor_super_property_expr(*alternate)),
            },
            Expr::Sequence(values) => Expr::Sequence(
                values
                    .into_iter()
                    .map(Self::rewrite_constructor_super_property_expr)
                    .collect(),
            ),
            Expr::Member { object, property } => Expr::Member {
                object: Box::new(Self::rewrite_constructor_super_property_member_base(
                    *object,
                )),
                property,
            },
            Expr::MemberComputed { object, property } => Expr::MemberComputed {
                object: Box::new(Self::rewrite_constructor_super_property_member_base(
                    *object,
                )),
                property: Box::new(Self::rewrite_constructor_super_property_expr(*property)),
            },
            Expr::Call { callee, arguments } => {
                let callee = match *callee {
                    Expr::Identifier(Identifier(name)) if name == "super" => {
                        Expr::Identifier(Identifier(name))
                    }
                    other => Self::rewrite_constructor_super_property_expr(other),
                };
                Expr::Call {
                    callee: Box::new(callee),
                    arguments: arguments
                        .into_iter()
                        .map(Self::rewrite_constructor_super_property_expr)
                        .collect(),
                }
            }
            Expr::New { callee, arguments } => Expr::New {
                callee: Box::new(Self::rewrite_constructor_super_property_expr(*callee)),
                arguments: arguments
                    .into_iter()
                    .map(Self::rewrite_constructor_super_property_expr)
                    .collect(),
            },
            Expr::Binary { op, left, right } => Expr::Binary {
                op,
                left: Box::new(Self::rewrite_constructor_super_property_expr(*left)),
                right: Box::new(Self::rewrite_constructor_super_property_expr(*right)),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target,
                value: Box::new(Self::rewrite_constructor_super_property_expr(*value)),
            },
            Expr::AssignMember {
                object,
                property,
                value,
            } => Expr::AssignMember {
                object: Box::new(Self::rewrite_constructor_super_property_member_base(
                    *object,
                )),
                property,
                value: Box::new(Self::rewrite_constructor_super_property_expr(*value)),
            },
            Expr::AssignMemberComputed {
                object,
                property,
                value,
            } => Expr::AssignMemberComputed {
                object: Box::new(Self::rewrite_constructor_super_property_member_base(
                    *object,
                )),
                property: Box::new(Self::rewrite_constructor_super_property_expr(*property)),
                value: Box::new(Self::rewrite_constructor_super_property_expr(*value)),
            },
            Expr::Update {
                target,
                increment,
                prefix,
            } => Expr::Update {
                target: match target {
                    UpdateTarget::Identifier(identifier) => UpdateTarget::Identifier(identifier),
                    UpdateTarget::Member { object, property } => UpdateTarget::Member {
                        object: Box::new(Self::rewrite_constructor_super_property_member_base(
                            *object,
                        )),
                        property,
                    },
                    UpdateTarget::MemberComputed { object, property } => {
                        UpdateTarget::MemberComputed {
                            object: Box::new(Self::rewrite_constructor_super_property_member_base(
                                *object,
                            )),
                            property: Box::new(Self::rewrite_constructor_super_property_expr(
                                *property,
                            )),
                        }
                    }
                },
                increment,
                prefix,
            },
            Expr::AnnexBCallAssignmentTarget { target } => Expr::AnnexBCallAssignmentTarget {
                target: Box::new(Self::rewrite_constructor_super_property_expr(*target)),
            },
            Expr::SpreadArgument(value) => Expr::SpreadArgument(Box::new(
                Self::rewrite_constructor_super_property_expr(*value),
            )),
        }
    }

    fn rewrite_constructor_super_property_member_base(object: Expr) -> Expr {
        let rewritten = Self::rewrite_constructor_super_property_expr(object);
        match rewritten {
            Expr::Identifier(Identifier(name)) if name == "super" => {
                Expr::Identifier(Identifier(CLASS_CONSTRUCTOR_SUPER_BASE_BINDING.to_string()))
            }
            other => other,
        }
    }

    fn mark_class_method_non_constructible(&self, method_value: Expr) -> Expr {
        let method_ident = Identifier("$__class_method_fn".to_string());
        Expr::Call {
            callee: Box::new(Expr::Function {
                name: None,
                params: vec![method_ident.clone()],
                body: vec![
                    Stmt::Expression(Expr::AssignMember {
                        object: Box::new(Expr::Identifier(method_ident.clone())),
                        property: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                        value: Box::new(Expr::Bool(true)),
                    }),
                    Stmt::Return(Some(Expr::Identifier(method_ident))),
                ],
            }),
            arguments: vec![method_value],
        }
    }

    fn class_method_key_is_prototype(&self, key: &ClassMethodKey) -> bool {
        match key {
            ClassMethodKey::Static(name) => name == "prototype",
            ClassMethodKey::Computed(Expr::String(StringLiteral {
                value: name,
                has_escape: false,
            })) => name == "prototype",
            _ => false,
        }
    }

    fn consume_balanced_brace_block(
        &mut self,
        start_error: &str,
        end_error: &str,
    ) -> Result<(), ParseError> {
        self.expect(TokenKind::LBrace, start_error)?;
        let mut depth = 1usize;
        while depth > 0 {
            let token = self.current().ok_or(ParseError {
                message: end_error.to_string(),
                position: self.last_position(),
            })?;
            match token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Eof => {
                    return Err(ParseError {
                        message: end_error.to_string(),
                        position: token.span.start,
                    });
                }
                _ => {}
            }
            self.advance();
        }
        Ok(())
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.current().cloned().ok_or(ParseError {
            message: "unexpected end of input".to_string(),
            position: 0,
        })?;
        let kind = token.kind.clone();
        let position = token.span.start;

        match kind {
            TokenKind::Number(value) => {
                self.advance();
                Ok(Expr::Number(value))
            }
            TokenKind::String(value) => {
                let has_escape = self
                    .source
                    .get(token.span.start..token.span.end)
                    .map(|slice| slice.contains('\\'))
                    .unwrap_or(false);
                self.advance();
                Ok(Expr::String(StringLiteral { value, has_escape }))
            }
            TokenKind::TemplatePart { .. } => self.parse_template_literal_expression(),
            TokenKind::Identifier(name) => {
                if self.identifier_token_matches_keyword(&token, "async")
                    && self.check_next_keyword("function")
                    && !self.has_line_terminator_between_tokens(self.pos, self.pos + 1)
                {
                    self.advance();
                    self.advance();
                    return self.parse_function_expression_after_keyword(true);
                }
                self.advance();
                match name.as_str() {
                    "true" if self.identifier_token_matches_keyword(&token, "true") => {
                        Ok(Expr::Bool(true))
                    }
                    "false" if self.identifier_token_matches_keyword(&token, "false") => {
                        Ok(Expr::Bool(false))
                    }
                    "null" if self.identifier_token_matches_keyword(&token, "null") => {
                        Ok(Expr::Null)
                    }
                    "this" if self.identifier_token_matches_keyword(&token, "this") => {
                        Ok(Expr::This)
                    }
                    "function" if self.identifier_token_matches_keyword(&token, "function") => {
                        self.parse_function_expression_after_keyword(false)
                    }
                    "class" if self.identifier_token_matches_keyword(&token, "class") => {
                        self.parse_class_expression_after_keyword()
                    }
                    _ if self.identifier_token_is_raw_name(&token, &name)
                        && is_forbidden_identifier_reference(&name)
                        && !(self.allow_super_reference && name == "super") =>
                    {
                        Err(ParseError {
                            message: "reserved word cannot be identifier reference".to_string(),
                            position,
                        })
                    }
                    _ => Ok(Expr::Identifier(Identifier(name))),
                }
            }
            TokenKind::LParen => {
                self.advance();
                let mut expressions = vec![self.parse_expression_inner()?];
                while self.matches(&TokenKind::Comma) {
                    expressions.push(self.parse_expression_inner()?);
                }
                self.expect(TokenKind::RParen, "expected ')' after expression")?;
                if expressions.len() == 1 {
                    Ok(expressions
                        .pop()
                        .expect("parenthesized expression should exist"))
                } else {
                    Ok(Expr::Sequence(expressions))
                }
            }
            TokenKind::Slash => self.parse_regex_literal(),
            TokenKind::LBrace => self.parse_object_literal(),
            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::Plus
            | TokenKind::PlusEqual
            | TokenKind::PlusPlus
            | TokenKind::Minus
            | TokenKind::MinusEqual
            | TokenKind::MinusMinus
            | TokenKind::Star
            | TokenKind::StarEqual
            | TokenKind::Bang
            | TokenKind::Tilde
            | TokenKind::Equal
            | TokenKind::SlashEqual
            | TokenKind::Percent
            | TokenKind::PercentEqual
            | TokenKind::Amp
            | TokenKind::AmpEqual
            | TokenKind::Pipe
            | TokenKind::PipeEqual
            | TokenKind::Caret
            | TokenKind::CaretEqual
            | TokenKind::EqualEqual
            | TokenKind::EqualEqualEqual
            | TokenKind::BangEqual
            | TokenKind::BangEqualEqual
            | TokenKind::Less
            | TokenKind::LessLess
            | TokenKind::LessLessEqual
            | TokenKind::LessEqual
            | TokenKind::Greater
            | TokenKind::GreaterGreater
            | TokenKind::GreaterGreaterEqual
            | TokenKind::GreaterGreaterGreater
            | TokenKind::GreaterGreaterGreaterEqual
            | TokenKind::GreaterEqual
            | TokenKind::AndAnd
            | TokenKind::AndAndEqual
            | TokenKind::OrOr
            | TokenKind::OrOrEqual
            | TokenKind::QuestionQuestion
            | TokenKind::QuestionQuestionEqual
            | TokenKind::Ellipsis
            | TokenKind::Dot
            | TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::Question => Err(ParseError {
                message: "unexpected operator at expression start".to_string(),
                position,
            }),
            TokenKind::Semicolon => Err(ParseError {
                message: "unexpected ';'".to_string(),
                position,
            }),
            TokenKind::RParen => Err(ParseError {
                message: "unexpected ')'".to_string(),
                position,
            }),
            TokenKind::RBrace => Err(ParseError {
                message: "unexpected '}'".to_string(),
                position,
            }),
            TokenKind::RBracket => Err(ParseError {
                message: "unexpected ']'".to_string(),
                position,
            }),
            TokenKind::Eof => Err(ParseError {
                message: "empty input".to_string(),
                position,
            }),
        }
    }

    fn parse_regex_literal(&mut self) -> Result<Expr, ParseError> {
        let start = self.current().map_or(0usize, |token| token.span.start);
        let bytes = self.source.as_bytes();
        if start >= bytes.len() || bytes[start] != b'/' {
            return Err(ParseError {
                message: "expected regular expression literal".to_string(),
                position: start,
            });
        }

        let mut pos = start + 1;
        let mut pattern = String::new();
        let mut in_character_class = false;
        let source_len = self.source.len();

        while pos < source_len {
            let tail = self.source.get(pos..).ok_or(ParseError {
                message: "unterminated regular expression literal".to_string(),
                position: start,
            })?;
            let ch = tail.chars().next().ok_or(ParseError {
                message: "unterminated regular expression literal".to_string(),
                position: start,
            })?;

            if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                return Err(ParseError {
                    message: "unterminated regular expression literal".to_string(),
                    position: start,
                });
            }

            if ch == '\\' {
                pattern.push(ch);
                pos += ch.len_utf8();
                let escaped_tail = self.source.get(pos..).ok_or(ParseError {
                    message: "unterminated regular expression literal".to_string(),
                    position: start,
                })?;
                let escaped = escaped_tail.chars().next().ok_or(ParseError {
                    message: "unterminated regular expression literal".to_string(),
                    position: start,
                })?;
                pattern.push(escaped);
                pos += escaped.len_utf8();
                continue;
            }

            if ch == '[' {
                in_character_class = true;
                pattern.push(ch);
                pos += ch.len_utf8();
                continue;
            }
            if ch == ']' {
                in_character_class = false;
                pattern.push(ch);
                pos += ch.len_utf8();
                continue;
            }
            if ch == '/' && !in_character_class {
                break;
            }

            pattern.push(ch);
            pos += ch.len_utf8();
        }

        if pos >= source_len || self.source.as_bytes()[pos] != b'/' {
            return Err(ParseError {
                message: "unterminated regular expression literal".to_string(),
                position: start,
            });
        }
        pos += 1;

        let mut flags = String::new();
        while pos < source_len {
            let tail = self.source.get(pos..).ok_or(ParseError {
                message: "invalid regular expression flags".to_string(),
                position: pos,
            })?;
            let ch = tail.chars().next().ok_or(ParseError {
                message: "invalid regular expression flags".to_string(),
                position: pos,
            })?;
            if !ch.is_ascii_alphabetic() {
                break;
            }
            flags.push(ch);
            pos += ch.len_utf8();
        }

        while let Some(token) = self.current() {
            if token.span.start < pos {
                self.advance();
                continue;
            }
            break;
        }

        Ok(Expr::RegexLiteral { pattern, flags })
    }

    fn parse_template_literal_expression(&mut self) -> Result<Expr, ParseError> {
        let (quasis, _raw_quasis, invalid_quasis, expressions) =
            self.parse_template_literal_parts()?;
        if quasis.is_empty() {
            return Ok(Expr::String(StringLiteral {
                value: String::new(),
                has_escape: false,
            }));
        }
        if quasis.len() != expressions.len() + 1 {
            return Err(self.error_current("invalid template literal shape"));
        }
        if invalid_quasis.iter().any(|invalid| *invalid) {
            return Err(self.error_current("invalid escape in template literal"));
        }

        let mut expr = Expr::String(quasis[0].clone());
        for (index, substitution) in expressions.into_iter().enumerate() {
            expr = Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(expr),
                right: Box::new(substitution),
            };
            expr = Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(expr),
                right: Box::new(Expr::String(quasis[index + 1].clone())),
            };
        }
        Ok(expr)
    }

    fn parse_tagged_template_call(&mut self, callee: Expr) -> Result<Expr, ParseError> {
        let (quasis, raw_quasis, invalid_quasis, expressions) =
            self.parse_template_literal_parts()?;
        if quasis.is_empty() {
            return Err(self.error_current("invalid tagged template literal"));
        }
        if quasis.len() != expressions.len() + 1 {
            return Err(self.error_current("invalid tagged template literal"));
        }
        let site_id = next_tagged_template_site_id();
        let mut arguments = Vec::with_capacity(expressions.len() + 1);
        arguments.push(self.build_tagged_template_object(
            site_id,
            quasis,
            raw_quasis,
            invalid_quasis,
        ));
        arguments.extend(expressions);
        Ok(Expr::Call {
            callee: Box::new(callee),
            arguments,
        })
    }

    fn build_tagged_template_object(
        &self,
        site_id: u64,
        quasis: Vec<StringLiteral>,
        raw_quasis: Vec<StringLiteral>,
        invalid_quasis: Vec<bool>,
    ) -> Expr {
        let cooked_quasis = quasis
            .into_iter()
            .zip(invalid_quasis)
            .map(|(quasi, invalid)| {
                if invalid {
                    Expr::Identifier(Identifier("undefined".to_string()))
                } else {
                    Expr::String(quasi)
                }
            })
            .collect();
        Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                property: "__getTemplateObject".to_string(),
            }),
            arguments: vec![
                Expr::Number(site_id as f64),
                Expr::ArrayLiteral(cooked_quasis),
                Expr::ArrayLiteral(raw_quasis.into_iter().map(Expr::String).collect()),
            ],
        }
    }

    fn parse_template_literal_parts(&mut self) -> Result<ParsedTemplateLiteralParts, ParseError> {
        let mut quasis = Vec::new();
        let mut raw_quasis = Vec::new();
        let mut invalid_quasis = Vec::new();
        let mut expressions = Vec::new();

        loop {
            let token = self.current().cloned().ok_or(ParseError {
                message: "unexpected end of input".to_string(),
                position: self.last_position(),
            })?;
            let (cooked, raw, has_escape, invalid_escape, tail) = match token.kind {
                TokenKind::TemplatePart {
                    cooked,
                    raw,
                    has_escape,
                    invalid_escape,
                    tail,
                } => (cooked, raw, has_escape, invalid_escape, tail),
                _ => {
                    return Err(ParseError {
                        message: "expected template literal".to_string(),
                        position: token.span.start,
                    });
                }
            };
            self.advance();
            quasis.push(StringLiteral {
                value: cooked,
                has_escape,
            });
            raw_quasis.push(StringLiteral {
                value: raw,
                has_escape,
            });
            invalid_quasis.push(invalid_escape);
            if tail {
                break;
            }

            let expr = self.parse_expression_with_commas()?;
            expressions.push(expr);
            self.expect(TokenKind::RBrace, "expected '}' after template expression")?;
        }

        Ok((quasis, raw_quasis, invalid_quasis, expressions))
    }

    fn parse_object_literal(&mut self) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LBrace, "expected '{' before object literal")?;
        let mut properties = Vec::new();
        if self.matches(&TokenKind::RBrace) {
            return Ok(Expr::ObjectLiteral(properties));
        }
        loop {
            let is_generator_method = self.matches(&TokenKind::Star);
            let mut key = self.parse_object_property_key()?;
            if let Some((accessor_key, accessor_value)) = self.try_parse_object_accessor(&key)? {
                if is_generator_method {
                    return Err(
                        self.error_current("unexpected '*' before accessor in object literal")
                    );
                }
                properties.push(ObjectProperty {
                    key: accessor_key,
                    value: accessor_value,
                });
                if self.matches(&TokenKind::Comma) {
                    if self.check(&TokenKind::RBrace) {
                        break;
                    }
                    continue;
                }
                break;
            }
            let value = if is_generator_method {
                self.parse_object_method_value(true)?
            } else if self.matches(&TokenKind::Colon) {
                if matches!(&key, ObjectPropertyKey::Static(name) if name == "__proto__") {
                    key = ObjectPropertyKey::ProtoSetter;
                }
                self.parse_expression_inner()?
            } else if self.check(&TokenKind::LParen) {
                self.parse_object_method_value(false)?
            } else {
                match &key {
                    ObjectPropertyKey::Static(name) => {
                        if is_forbidden_identifier_reference(name) {
                            return Err(ParseError {
                                message: "reserved word cannot be identifier reference".to_string(),
                                position: self.previous_position(),
                            });
                        }
                        let identifier = Identifier(name.clone());
                        if self.matches(&TokenKind::Equal) {
                            let initializer = self.parse_assignment()?;
                            Expr::Assign {
                                target: identifier,
                                value: Box::new(initializer),
                            }
                        } else {
                            Expr::Identifier(identifier)
                        }
                    }
                    ObjectPropertyKey::Computed(_) => {
                        return Err(self.error_current(
                            "expected ':' after computed property name in object literal",
                        ));
                    }
                    ObjectPropertyKey::AccessorGet(_)
                    | ObjectPropertyKey::AccessorSet(_)
                    | ObjectPropertyKey::AccessorGetComputed(_)
                    | ObjectPropertyKey::AccessorSetComputed(_) => {
                        return Err(self.error_current("unexpected accessor in object literal"));
                    }
                    ObjectPropertyKey::ProtoSetter => {
                        return Err(
                            self.error_current("unexpected __proto__ setter in object literal")
                        );
                    }
                }
            };
            properties.push(ObjectProperty { key, value });
            if self.matches(&TokenKind::Comma) {
                if self.check(&TokenKind::RBrace) {
                    break;
                }
                continue;
            }
            break;
        }
        self.expect(TokenKind::RBrace, "expected '}' after object literal")?;
        Ok(Expr::ObjectLiteral(properties))
    }

    fn try_parse_object_accessor(
        &mut self,
        key: &ObjectPropertyKey,
    ) -> Result<Option<(ObjectPropertyKey, Expr)>, ParseError> {
        let accessor_kind = match key {
            ObjectPropertyKey::Static(name) if name == "get" => Some("get"),
            ObjectPropertyKey::Static(name) if name == "set" => Some("set"),
            _ => None,
        };
        let Some(accessor_kind) = accessor_kind else {
            return Ok(None);
        };

        let accessor_key = if self.matches(&TokenKind::LBracket) {
            let key_expr = self.parse_expression_with_in()?;
            self.expect(
                TokenKind::RBracket,
                "expected ']' after computed property name",
            )?;
            if accessor_kind == "get" {
                ObjectPropertyKey::AccessorGetComputed(Box::new(key_expr))
            } else {
                ObjectPropertyKey::AccessorSetComputed(Box::new(key_expr))
            }
        } else {
            if !self.check_next(&TokenKind::LParen) {
                return Ok(None);
            }
            let accessor_name = match self.current().map(|token| token.kind.clone()) {
                Some(TokenKind::Identifier(name)) | Some(TokenKind::String(name)) => {
                    self.advance();
                    name
                }
                Some(TokenKind::Number(number)) => {
                    self.advance();
                    Self::number_property_key(number)
                }
                _ => return Ok(None),
            };
            if accessor_kind == "get" {
                ObjectPropertyKey::AccessorGet(accessor_name)
            } else {
                ObjectPropertyKey::AccessorSet(accessor_name)
            }
        };

        self.expect(TokenKind::LParen, "expected '(' after accessor name")?;
        let (params, simple_parameters, default_initializers, pattern_effects) =
            self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        let mut body = self.parse_function_body_with_super_policy(
            "expected '{' before function body",
            "expected '}' after function body",
            true,
            false,
        )?;
        self.prepend_parameter_initializers(&mut body, &default_initializers, &pattern_effects);
        if !simple_parameters {
            self.prepend_non_simple_params_marker(&mut body);
        }
        self.insert_no_prototype_marker(&mut body);

        Ok(Some((
            accessor_key,
            Expr::Function {
                name: None,
                params,
                body,
            },
        )))
    }

    fn parse_object_property_key(&mut self) -> Result<ObjectPropertyKey, ParseError> {
        let token = self.current().ok_or(ParseError {
            message: "expected property name in object literal".to_string(),
            position: self.last_position(),
        })?;
        match token.kind.clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(ObjectPropertyKey::Static(name))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(ObjectPropertyKey::Static(name))
            }
            TokenKind::Number(number) => {
                self.advance();
                let key = Self::number_property_key(number);
                Ok(ObjectPropertyKey::Static(key))
            }
            TokenKind::LBracket => {
                self.advance();
                let expr = self.parse_expression_with_in()?;
                self.expect(
                    TokenKind::RBracket,
                    "expected ']' after computed property name",
                )?;
                Ok(ObjectPropertyKey::Computed(Box::new(expr)))
            }
            _ => Err(ParseError {
                message: "expected property name in object literal".to_string(),
                position: token.span.start,
            }),
        }
    }

    fn parse_object_method_value(&mut self, is_generator: bool) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after method name")?;
        let (params, simple_parameters, default_initializers, pattern_effects) =
            self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        let mut body = self.parse_function_body_with_super_policy(
            "expected '{' before method body",
            "expected '}' after method body",
            true,
            is_generator,
        )?;
        self.prepend_parameter_initializers(&mut body, &default_initializers, &pattern_effects);
        if !simple_parameters {
            self.prepend_non_simple_params_marker(&mut body);
        }
        if is_generator {
            self.insert_generator_function_marker(&mut body);
        }
        self.insert_no_prototype_marker(&mut body);
        Ok(Expr::Function {
            name: None,
            params,
            body,
        })
    }

    fn parse_array_literal(&mut self) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LBracket, "expected '[' before array literal")?;
        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) {
            if self.matches(&TokenKind::Comma) {
                elements.push(Expr::Elision);
                continue;
            }
            let is_spread = self.matches(&TokenKind::Ellipsis);
            let element = self.parse_expression_inner()?;
            if is_spread {
                elements.push(Expr::SpreadArgument(Box::new(element)));
            } else {
                elements.push(element);
            }
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RBracket, "expected ']' after array literal")?;
        Ok(Expr::ArrayLiteral(elements))
    }

    fn expect_identifier_name(&mut self, message: &str) -> Result<String, ParseError> {
        let token = self.current().ok_or(ParseError {
            message: message.to_string(),
            position: self.last_position(),
        })?;
        if let TokenKind::Identifier(name) = &token.kind {
            let cloned = name.clone();
            self.advance();
            Ok(cloned)
        } else {
            Err(ParseError {
                message: message.to_string(),
                position: token.span.start,
            })
        }
    }

    fn expect_binding_identifier(&mut self, message: &str) -> Result<String, ParseError> {
        let token = self.current().ok_or(ParseError {
            message: message.to_string(),
            position: self.last_position(),
        })?;
        if let TokenKind::Identifier(name) = &token.kind {
            if self.identifier_token_is_raw_name(token, name)
                && is_forbidden_binding_identifier(name)
            {
                return Err(ParseError {
                    message: message.to_string(),
                    position: token.span.start,
                });
            }
            let cloned = name.clone();
            self.advance();
            Ok(cloned)
        } else {
            Err(ParseError {
                message: message.to_string(),
                position: token.span.start,
            })
        }
    }

    fn expect_var_binding_identifier(&mut self, message: &str) -> Result<String, ParseError> {
        let token = self.current().ok_or(ParseError {
            message: message.to_string(),
            position: self.last_position(),
        })?;
        if let TokenKind::Identifier(name) = &token.kind {
            if self.identifier_token_is_raw_name(token, name)
                && is_forbidden_binding_identifier(name)
                && name != "let"
            {
                return Err(ParseError {
                    message: message.to_string(),
                    position: token.span.start,
                });
            }
            let cloned = name.clone();
            self.advance();
            Ok(cloned)
        } else {
            Err(ParseError {
                message: message.to_string(),
                position: token.span.start,
            })
        }
    }

    fn matches_keyword(&mut self, keyword: &str) -> bool {
        match self.current() {
            Some(token) if self.identifier_token_matches_keyword(token, keyword) => {
                self.advance();
                true
            }
            None => false,
            _ => false,
        }
    }

    fn check_keyword(&self, keyword: &str) -> bool {
        self.current()
            .is_some_and(|token| self.identifier_token_matches_keyword(token, keyword))
    }

    fn check_identifier(&self) -> bool {
        matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::Identifier(_))
        )
    }

    fn identifier_text_can_be_binding_name(&self, text: &str) -> bool {
        !is_forbidden_binding_identifier(text)
    }

    fn check_template_part(&self) -> bool {
        matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::TemplatePart { .. })
        )
    }

    fn check_next(&self, expected: &TokenKind) -> bool {
        matches!(self.tokens.get(self.pos + 1), Some(token) if &token.kind == expected)
    }

    fn check_next_keyword(&self, keyword: &str) -> bool {
        self.tokens
            .get(self.pos + 1)
            .is_some_and(|token| self.identifier_token_matches_keyword(token, keyword))
    }

    fn check_nth(&self, offset: usize, expected: &TokenKind) -> bool {
        matches!(
            self.tokens.get(self.pos + offset),
            Some(token) if &token.kind == expected
        )
    }

    fn check_nth_keyword(&self, offset: usize, keyword: &str) -> bool {
        self.tokens
            .get(self.pos + offset)
            .is_some_and(|token| self.identifier_token_matches_keyword(token, keyword))
    }

    fn check(&self, expected: &TokenKind) -> bool {
        matches!(self.current(), Some(token) if &token.kind == expected)
    }

    fn matches(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: TokenKind, message: &str) -> Result<(), ParseError> {
        let token = self.current().ok_or(ParseError {
            message: message.to_string(),
            position: self.last_position(),
        })?;
        if token.kind == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                message: message.to_string(),
                position: token.span.start,
            })
        }
    }

    fn expect_eof(&mut self) -> Result<(), ParseError> {
        let token = self.current().ok_or(ParseError {
            message: "unexpected end of token stream".to_string(),
            position: self.last_position(),
        })?;
        if token.kind == TokenKind::Eof {
            Ok(())
        } else {
            Err(ParseError {
                message: "unexpected trailing input".to_string(),
                position: token.span.start,
            })
        }
    }

    fn is_eof(&self) -> bool {
        self.check(&TokenKind::Eof)
    }

    fn error_current(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            position: self.current_position(),
        }
    }

    fn current_position(&self) -> usize {
        self.current()
            .map(|token| token.span.start)
            .unwrap_or_else(|| self.last_position())
    }

    fn previous_position(&self) -> usize {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|token| token.span.start)
            .unwrap_or_default()
    }

    fn has_line_terminator_between_prev_and_current(&self) -> bool {
        if self.pos == 0 {
            return false;
        }
        self.has_line_terminator_between_tokens(self.pos.saturating_sub(1), self.pos)
    }

    fn has_line_terminator_between_tokens(&self, left_index: usize, right_index: usize) -> bool {
        let left_end = self
            .tokens
            .get(left_index)
            .map(|token| token.span.end)
            .unwrap_or_default();
        let right_start = self
            .tokens
            .get(right_index)
            .map(|token| token.span.start)
            .unwrap_or_default();
        self.has_line_terminator_between_offsets(left_end, right_start)
    }

    fn has_line_terminator_between_offsets(&self, left_end: usize, right_start: usize) -> bool {
        if right_start <= left_end || right_start > self.source.len() {
            return false;
        }
        self.source[left_end..right_start]
            .chars()
            .any(|ch| matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    }

    fn identifier_token_matches_keyword(&self, token: &Token, keyword: &str) -> bool {
        matches!(&token.kind, TokenKind::Identifier(name) if name == keyword)
            && self.identifier_token_is_raw_name(token, keyword)
    }

    fn identifier_token_is_raw_name(&self, token: &Token, name: &str) -> bool {
        self.source
            .get(token.span.start..token.span.end)
            .is_some_and(|raw| raw == name)
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn last_position(&self) -> usize {
        self.tokens
            .last()
            .map(|token| token.span.end)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CLASS_METHOD_NO_PROTOTYPE_MARKER, GENERATOR_FUNCTION_MARKER, parse_expression,
        parse_module, parse_script,
    };
    use ast::{
        BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier, ModuleExport,
        ModuleImportBinding, ObjectProperty, ObjectPropertyKey, Script, Stmt, StringLiteral,
        SwitchCase, UnaryOp, UpdateTarget, VariableDeclaration,
    };

    #[test]
    fn parses_additive_expression() {
        let parsed = parse_expression("1 + 2 - 3").expect("parser should succeed");
        let add = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::Number(2.0)),
        };
        let expected = Expr::Binary {
            op: BinaryOp::Sub,
            left: Box::new(add),
            right: Box::new(Expr::Number(3.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn module_parse_baseline() {
        let source = "\
import value from './dep.js';\n\
import { inc as plus } from './math.js';\n\
const local = plus + 1;\n\
export { local };\n\
export default value;\n";
        let parsed = parse_module(source).expect("module parsing should succeed");
        assert_eq!(parsed.imports.len(), 2);
        assert_eq!(parsed.imports[0].specifier, "./dep.js");
        assert_eq!(
            parsed.imports[0].bindings,
            vec![ModuleImportBinding {
                imported: "default".to_string(),
                local: "value".to_string(),
            }]
        );
        assert_eq!(parsed.imports[1].specifier, "./math.js");
        assert_eq!(
            parsed.imports[1].bindings,
            vec![ModuleImportBinding {
                imported: "inc".to_string(),
                local: "plus".to_string(),
            }]
        );
        assert!(parsed.exports.contains(&ModuleExport {
            exported: "local".to_string(),
            local: "local".to_string(),
        }));
        assert!(
            parsed
                .exports
                .iter()
                .any(|entry| entry.exported == "default")
        );
        assert!(
            matches!(
                parsed.body.statements.last(),
                Some(Stmt::Expression(Expr::ObjectLiteral(_)))
            ),
            "module parse should append synthetic export snapshot expression"
        );
    }

    #[test]
    fn parses_call_expression() {
        let parsed = parse_expression("add(1, mul(2, 3))").expect("parser should succeed");
        let expected = Expr::Call {
            callee: Box::new(Expr::Identifier(Identifier("add".to_string()))),
            arguments: vec![
                Expr::Number(1.0),
                Expr::Call {
                    callee: Box::new(Expr::Identifier(Identifier("mul".to_string()))),
                    arguments: vec![Expr::Number(2.0), Expr::Number(3.0)],
                },
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_parenthesized_comma_expression_baseline() {
        let parsed = parse_expression("(0, eval)").expect("parser should succeed");
        assert_eq!(
            parsed,
            Expr::Sequence(vec![
                Expr::Number(0.0),
                Expr::Identifier(Identifier("eval".to_string())),
            ])
        );
    }

    #[test]
    fn parses_object_literal_and_member_assignment() {
        let parsed =
            parse_expression("obj.value = { answer: 42, key }").expect("parser should succeed");
        let expected = Expr::AssignMember {
            object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
            property: "value".to_string(),
            value: Box::new(Expr::ObjectLiteral(vec![
                ObjectProperty {
                    key: ObjectPropertyKey::Static("answer".to_string()),
                    value: Expr::Number(42.0),
                },
                ObjectProperty {
                    key: ObjectPropertyKey::Static("key".to_string()),
                    value: Expr::Identifier(Identifier("key".to_string())),
                },
            ])),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_computed_property_and_method_in_object_literal() {
        let parsed =
            parse_expression("({ [v]: 1, f() { return 1; } })").expect("parser should succeed");
        let expected = Expr::ObjectLiteral(vec![
            ObjectProperty {
                key: ObjectPropertyKey::Computed(Box::new(Expr::Identifier(Identifier(
                    "v".to_string(),
                )))),
                value: Expr::Number(1.0),
            },
            ObjectProperty {
                key: ObjectPropertyKey::Static("f".to_string()),
                value: Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![
                        Stmt::Expression(Expr::String(StringLiteral {
                            value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                            has_escape: false,
                        })),
                        Stmt::Return(Some(Expr::Number(1.0))),
                    ],
                },
            },
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_generator_methods_in_object_literal_baseline() {
        let parsed = parse_expression("({ *f() { return 1; }, *[k]() { return 2; } })")
            .expect("parser should succeed");
        let expected = Expr::ObjectLiteral(vec![
            ObjectProperty {
                key: ObjectPropertyKey::Static("f".to_string()),
                value: Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![
                        Stmt::Expression(Expr::String(StringLiteral {
                            value: GENERATOR_FUNCTION_MARKER.to_string(),
                            has_escape: false,
                        })),
                        Stmt::Expression(Expr::String(StringLiteral {
                            value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                            has_escape: false,
                        })),
                        Stmt::Return(Some(Expr::Number(1.0))),
                    ],
                },
            },
            ObjectProperty {
                key: ObjectPropertyKey::Computed(Box::new(Expr::Identifier(Identifier(
                    "k".to_string(),
                )))),
                value: Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![
                        Stmt::Expression(Expr::String(StringLiteral {
                            value: GENERATOR_FUNCTION_MARKER.to_string(),
                            has_escape: false,
                        })),
                        Stmt::Expression(Expr::String(StringLiteral {
                            value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                            has_escape: false,
                        })),
                        Stmt::Return(Some(Expr::Number(2.0))),
                    ],
                },
            },
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_class_static_generator_computed_name_baseline() {
        parse_script("class C { static *['prototype']() {} }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_getter_and_setter_in_object_literal() {
        let parsed =
            parse_expression("({ get foo() {}, set foo(v) {} })").expect("parser should succeed");
        let expected = Expr::ObjectLiteral(vec![
            ObjectProperty {
                key: ObjectPropertyKey::AccessorGet("foo".to_string()),
                value: Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![Stmt::Expression(Expr::String(StringLiteral {
                        value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                        has_escape: false,
                    }))],
                },
            },
            ObjectProperty {
                key: ObjectPropertyKey::AccessorSet("foo".to_string()),
                value: Expr::Function {
                    name: None,
                    params: vec![Identifier("v".to_string())],
                    body: vec![Stmt::Expression(Expr::String(StringLiteral {
                        value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                        has_escape: false,
                    }))],
                },
            },
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_computed_getter_and_setter_in_object_literal() {
        let parsed = parse_expression("({ get [foo]() {}, set [bar](v) {} })")
            .expect("parser should succeed");
        let expected = Expr::ObjectLiteral(vec![
            ObjectProperty {
                key: ObjectPropertyKey::AccessorGetComputed(Box::new(Expr::Identifier(
                    Identifier("foo".to_string()),
                ))),
                value: Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![Stmt::Expression(Expr::String(StringLiteral {
                        value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                        has_escape: false,
                    }))],
                },
            },
            ObjectProperty {
                key: ObjectPropertyKey::AccessorSetComputed(Box::new(Expr::Identifier(
                    Identifier("bar".to_string()),
                ))),
                value: Expr::Function {
                    name: None,
                    params: vec![Identifier("v".to_string())],
                    body: vec![Stmt::Expression(Expr::String(StringLiteral {
                        value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                        has_escape: false,
                    }))],
                },
            },
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_literal_named_getter_and_setter_in_object_literal() {
        let parsed =
            parse_expression("({ get 'x'() {}, set 0b101(v) {} })").expect("parser should succeed");
        let expected = Expr::ObjectLiteral(vec![
            ObjectProperty {
                key: ObjectPropertyKey::AccessorGet("x".to_string()),
                value: Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![Stmt::Expression(Expr::String(StringLiteral {
                        value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                        has_escape: false,
                    }))],
                },
            },
            ObjectProperty {
                key: ObjectPropertyKey::AccessorSet("5".to_string()),
                value: Expr::Function {
                    name: None,
                    params: vec![Identifier("v".to_string())],
                    body: vec![Stmt::Expression(Expr::String(StringLiteral {
                        value: CLASS_METHOD_NO_PROTOTYPE_MARKER.to_string(),
                        has_escape: false,
                    }))],
                },
            },
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_computed_object_accessor_with_in_in_for_init() {
        parse_script("for (obj = { get ['x' in empty]() {} }; ; ) break;")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_function_expression() {
        let parsed = parse_expression("function add(a, b) { return a + b; }")
            .expect("parser should succeed");
        let expected = Expr::Function {
            name: Some(Identifier("add".to_string())),
            params: vec![Identifier("a".to_string()), Identifier("b".to_string())],
            body: vec![
                Stmt::Expression(Expr::String(StringLiteral {
                    value: "$__qjs_named_function_expr__$".to_string(),
                    has_escape: false,
                })),
                Stmt::Return(Some(Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Identifier(Identifier("a".to_string()))),
                    right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
                })),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn inserts_named_function_expression_marker_after_use_strict() {
        let parsed = parse_expression("function add() { 'use strict'; return 1; }")
            .expect("parser should succeed");
        let Expr::Function { body, .. } = parsed else {
            panic!("expected function expression");
        };
        assert!(matches!(
            body.first(),
            Some(Stmt::Expression(Expr::String(StringLiteral { value, .. }))) if value == "use strict"
        ));
        assert!(matches!(
            body.get(1),
            Some(Stmt::Expression(Expr::String(StringLiteral { value, .. }))) if value == "$__qjs_named_function_expr__$"
        ));
    }

    #[test]
    fn parses_arrow_function_with_empty_parameters() {
        let parsed = parse_expression("() => 1").expect("parser should succeed");
        let expected = Expr::Function {
            name: None,
            params: vec![],
            body: vec![
                Stmt::Expression(Expr::String(StringLiteral {
                    value: "$__qjs_arrow_function__$".to_string(),
                    has_escape: false,
                })),
                Stmt::Return(Some(Expr::Number(1.0))),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_arrow_function_with_single_parameter() {
        let parsed = parse_expression("x => x + 1").expect("parser should succeed");
        let expected = Expr::Function {
            name: None,
            params: vec![Identifier("x".to_string())],
            body: vec![
                Stmt::Expression(Expr::String(StringLiteral {
                    value: "$__qjs_arrow_function__$".to_string(),
                    has_escape: false,
                })),
                Stmt::Return(Some(Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                    right: Box::new(Expr::Number(1.0)),
                })),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_arrow_function_with_default_parameters() {
        let parsed =
            parse_expression("(p = 1, arguments) => arguments").expect("parser should succeed");
        let expected = Expr::Function {
            name: None,
            params: vec![
                Identifier("p".to_string()),
                Identifier("arguments".to_string()),
            ],
            body: vec![
                Stmt::Expression(Expr::String(StringLiteral {
                    value: "$__qjs_arrow_function__$".to_string(),
                    has_escape: false,
                })),
                Stmt::Expression(Expr::String(StringLiteral {
                    value: "$__qjs_non_simple_params__$".to_string(),
                    has_escape: false,
                })),
                Stmt::Expression(Expr::String(StringLiteral {
                    value: "$__qjs_param_init_scope_start__$".to_string(),
                    has_escape: false,
                })),
                Stmt::If {
                    condition: Expr::Binary {
                        op: BinaryOp::StrictEqual,
                        left: Box::new(Expr::Identifier(Identifier("p".to_string()))),
                        right: Box::new(Expr::Identifier(Identifier("undefined".to_string()))),
                    },
                    consequent: Box::new(Stmt::Expression(Expr::Assign {
                        target: Identifier("p".to_string()),
                        value: Box::new(Expr::Number(1.0)),
                    })),
                    alternate: None,
                },
                Stmt::Expression(Expr::String(StringLiteral {
                    value: "$__qjs_param_init_scope_end__$".to_string(),
                    has_escape: false,
                })),
                Stmt::Return(Some(Expr::Identifier(Identifier("arguments".to_string())))),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_async_arrow_function_with_paren_parameters() {
        let parsed = parse_expression("async () => await 1").expect("parser should succeed");
        let Expr::Function { body, .. } = parsed else {
            panic!("expected function expression");
        };
        assert!(body.iter().any(|stmt| {
            matches!(
                stmt,
                Stmt::Expression(Expr::String(StringLiteral { value, .. }))
                    if value == "$__qjs_async_function__$"
            )
        }));
        assert!(body.iter().any(|stmt| {
            matches!(
                stmt,
                Stmt::Expression(Expr::String(StringLiteral { value, .. }))
                    if value == "$__qjs_arrow_function__$"
            )
        }));
    }

    #[test]
    fn parses_call_with_trailing_comma() {
        let parsed = parse_expression("f(1,)").expect("parser should succeed");
        let expected = Expr::Call {
            callee: Box::new(Expr::Identifier(Identifier("f".to_string()))),
            arguments: vec![Expr::Number(1.0)],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_call_with_spread_and_trailing_comma() {
        let parsed = parse_expression("foo(...[],)").expect("parser should succeed");
        let expected = Expr::Call {
            callee: Box::new(Expr::Identifier(Identifier("foo".to_string()))),
            arguments: vec![Expr::SpreadArgument(Box::new(Expr::ArrayLiteral(vec![])))],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn allows_reserved_word_as_property_name() {
        let parsed = parse_expression("obj.default").expect("parser should succeed");
        let expected = Expr::Member {
            object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
            property: "default".to_string(),
        };
        assert_eq!(parsed, expected);
        parse_expression("({ default: 1 })").expect("parser should succeed");
    }

    #[test]
    fn parses_computed_member_assignment() {
        let parsed = parse_expression("obj[key] = 1").expect("parser should succeed");
        let expected = Expr::AssignMemberComputed {
            object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
            property: Box::new(Expr::Identifier(Identifier("key".to_string()))),
            value: Box::new(Expr::Number(1.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_prefix_increment_expression() {
        let parsed = parse_expression("++x").expect("parser should succeed");
        let expected = Expr::Update {
            target: UpdateTarget::Identifier(Identifier("x".to_string())),
            increment: true,
            prefix: true,
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_postfix_increment_expression() {
        let parsed = parse_expression("x++").expect("parser should succeed");
        let expected = Expr::Update {
            target: UpdateTarget::Identifier(Identifier("x".to_string())),
            increment: true,
            prefix: false,
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn rejects_postfix_increment_with_line_terminator() {
        let err = parse_script("x\n++;").expect_err("parser should fail");
        assert!(err.message.starts_with("unexpected"));
    }

    #[test]
    fn parses_regular_expression_literal() {
        let parsed = parse_expression("/x/g").expect("parser should succeed");
        let expected = Expr::RegexLiteral {
            pattern: "x".to_string(),
            flags: "g".to_string(),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_template_literal_without_substitution() {
        let parsed = parse_expression("`ok`").expect("parser should succeed");
        assert_eq!(
            parsed,
            Expr::String(StringLiteral {
                value: "ok".to_string(),
                has_escape: false,
            })
        );
    }

    #[test]
    fn parses_template_literal_as_concatenation() {
        let parsed = parse_expression("`a${b}c`").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::String(StringLiteral {
                    value: "a".to_string(),
                    has_escape: false,
                })),
                right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
            }),
            right: Box::new(Expr::String(StringLiteral {
                value: "c".to_string(),
                has_escape: false,
            })),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_tagged_template_as_call() {
        let parsed = parse_expression("tag`a${b}`").expect("parser should succeed");
        let Expr::Call { callee, arguments } = parsed else {
            panic!("expected tagged template to lower into call expression");
        };
        assert_eq!(*callee, Expr::Identifier(Identifier("tag".to_string())));
        assert_eq!(arguments.len(), 2);
        assert_eq!(arguments[1], Expr::Identifier(Identifier("b".to_string())));

        let Expr::Call {
            callee: object_method,
            arguments: template_args,
        } = &arguments[0]
        else {
            panic!("expected first tagged template argument to be object helper call");
        };
        assert_eq!(
            object_method.as_ref(),
            &Expr::Member {
                object: Box::new(Expr::Identifier(Identifier("Object".to_string()))),
                property: "__getTemplateObject".to_string(),
            }
        );
        assert_eq!(template_args.len(), 3);
        assert!(matches!(template_args[0], Expr::Number(value) if value >= 1.0));
        assert_eq!(
            template_args[1],
            Expr::ArrayLiteral(vec![
                Expr::String(StringLiteral {
                    value: "a".to_string(),
                    has_escape: false,
                }),
                Expr::String(StringLiteral {
                    value: "".to_string(),
                    has_escape: false,
                }),
            ])
        );
        assert_eq!(
            template_args[2],
            Expr::ArrayLiteral(vec![
                Expr::String(StringLiteral {
                    value: "a".to_string(),
                    has_escape: false,
                }),
                Expr::String(StringLiteral {
                    value: "".to_string(),
                    has_escape: false,
                }),
            ])
        );
    }

    #[test]
    fn gives_tagged_template_precedence_over_new() {
        let parsed = parse_expression("new tag`value`").expect("parser should succeed");
        let Expr::New { callee, arguments } = parsed else {
            panic!("expected new expression");
        };
        assert!(
            arguments.is_empty(),
            "new expression should not have call arguments"
        );
        let Expr::Call {
            callee: tagged_callee,
            ..
        } = *callee
        else {
            panic!("expected new callee to be tagged template call");
        };
        assert_eq!(
            *tagged_callee,
            Expr::Identifier(Identifier("tag".to_string()))
        );
    }

    #[test]
    fn parses_new_expression() {
        let parsed = parse_expression("new f(1, 2)").expect("parser should succeed");
        let expected = Expr::New {
            callee: Box::new(Expr::Identifier(Identifier("f".to_string()))),
            arguments: vec![Expr::Number(1.0), Expr::Number(2.0)],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_member_chain_with_call() {
        let parsed = parse_expression("obj.fn(1).value").expect("parser should succeed");
        let expected = Expr::Member {
            object: Box::new(Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
                    property: "fn".to_string(),
                }),
                arguments: vec![Expr::Number(1.0)],
            }),
            property: "value".to_string(),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_unary_expression() {
        let parsed = parse_expression("!-x").expect("parser should succeed");
        let expected = Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(Expr::Identifier(Identifier("x".to_string()))),
            }),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_keyword_unary_expression() {
        let parsed = parse_expression("typeof void delete x").expect("parser should succeed");
        let expected = Expr::Unary {
            op: UnaryOp::Typeof,
            expr: Box::new(Expr::Unary {
                op: UnaryOp::Void,
                expr: Box::new(Expr::Unary {
                    op: UnaryOp::Delete,
                    expr: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                }),
            }),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_boolean_and_null_literals() {
        assert_eq!(
            parse_expression("true").expect("parser should succeed"),
            Expr::Bool(true)
        );
        assert_eq!(
            parse_expression("false").expect("parser should succeed"),
            Expr::Bool(false)
        );
        assert_eq!(
            parse_expression("null").expect("parser should succeed"),
            Expr::Null
        );
        assert_eq!(
            parse_expression("'ok'").expect("parser should succeed"),
            Expr::String(StringLiteral {
                value: "ok".to_string(),
                has_escape: false,
            })
        );
        assert_eq!(
            parse_expression("this").expect("parser should succeed"),
            Expr::This
        );
    }

    #[test]
    fn parses_array_literal() {
        let parsed = parse_expression("[1, 2, x]").expect("parser should succeed");
        let expected = Expr::ArrayLiteral(vec![
            Expr::Number(1.0),
            Expr::Number(2.0),
            Expr::Identifier(Identifier("x".to_string())),
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_array_literal_with_elisions() {
        let parsed = parse_expression("[1,,3,]").expect("parser should succeed");
        let expected =
            Expr::ArrayLiteral(vec![Expr::Number(1.0), Expr::Elision, Expr::Number(3.0)]);
        assert_eq!(parsed, expected);

        let parsed = parse_expression("[,]").expect("parser should succeed");
        assert_eq!(parsed, Expr::ArrayLiteral(vec![Expr::Elision]));
    }

    #[test]
    fn parses_array_literal_with_spread_elements() {
        let parsed = parse_expression("[1, ...items, 3]").expect("parser should succeed");
        let expected = Expr::ArrayLiteral(vec![
            Expr::Number(1.0),
            Expr::SpreadArgument(Box::new(Expr::Identifier(Identifier("items".to_string())))),
            Expr::Number(3.0),
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_comparison_expression() {
        let parsed = parse_expression("1 + 2 * 3 >= 7").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::GreaterEqual,
            left: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Number(1.0)),
                right: Box::new(Expr::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(Expr::Number(2.0)),
                    right: Box::new(Expr::Number(3.0)),
                }),
            }),
            right: Box::new(Expr::Number(7.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_in_expression() {
        let parsed = parse_expression("'arguments' in this").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::In,
            left: Box::new(Expr::String(StringLiteral {
                value: "arguments".to_string(),
                has_escape: false,
            })),
            right: Box::new(Expr::This),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_equality_after_in_with_correct_precedence() {
        let parsed = parse_expression("true in object !== \"true\" in object")
            .expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::StrictNotEqual,
            left: Box::new(Expr::Binary {
                op: BinaryOp::In,
                left: Box::new(Expr::Bool(true)),
                right: Box::new(Expr::Identifier(Identifier("object".to_string()))),
            }),
            right: Box::new(Expr::Binary {
                op: BinaryOp::In,
                left: Box::new(Expr::String(StringLiteral {
                    value: "true".to_string(),
                    has_escape: false,
                })),
                right: Box::new(Expr::Identifier(Identifier("object".to_string()))),
            }),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_instanceof_expression() {
        let parsed = parse_expression("value instanceof Foo").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::InstanceOf,
            left: Box::new(Expr::Identifier(Identifier("value".to_string()))),
            right: Box::new(Expr::Identifier(Identifier("Foo".to_string()))),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_strict_equality_expression() {
        let parsed = parse_expression("1 === 1 !== 0").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::StrictNotEqual,
            left: Box::new(Expr::Binary {
                op: BinaryOp::StrictEqual,
                left: Box::new(Expr::Number(1.0)),
                right: Box::new(Expr::Number(1.0)),
            }),
            right: Box::new(Expr::Number(0.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_conditional_consequent_with_in_when_forbidding_in_elsewhere() {
        let script = "for (true ? '' in cond1() : cond2(); false; ) ;";
        let parsed = parse_script(script).expect("parser should succeed");
        assert!(!parsed.statements.is_empty());
    }

    #[test]
    fn parses_logical_expression_with_precedence() {
        let parsed = parse_expression("a && b || c").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::LogicalOr,
            left: Box::new(Expr::Binary {
                op: BinaryOp::LogicalAnd,
                left: Box::new(Expr::Identifier(Identifier("a".to_string()))),
                right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
            }),
            right: Box::new(Expr::Identifier(Identifier("c".to_string()))),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_nullish_coalesce_expression_baseline() {
        let parsed = parse_expression("a ?? b").expect("parser should succeed");
        assert!(matches!(parsed, Expr::Call { .. }));
    }

    #[test]
    fn parses_logical_assignment_operators_baseline() {
        parse_script("var x = 1; x &&= 2; x ||= 3; x ??= 4;")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_object_assignment_pattern_with_default_initializer() {
        parse_script("var x, count = 0; ({x = (count = count + 1)} = {x: 1});")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_function_declaration_and_return() {
        let parsed = parse_script("function add(a, b) { return a + b; } add(1, 2);")
            .expect("script parsing should succeed");
        let expected = Script {
            statements: vec![
                Stmt::FunctionDeclaration(FunctionDeclaration {
                    name: Identifier("add".to_string()),
                    params: vec![Identifier("a".to_string()), Identifier("b".to_string())],
                    body: vec![Stmt::Return(Some(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Identifier(Identifier("a".to_string()))),
                        right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
                    }))],
                }),
                Stmt::Expression(Expr::Call {
                    callee: Box::new(Expr::Identifier(Identifier("add".to_string()))),
                    arguments: vec![Expr::Number(1.0), Expr::Number(2.0)],
                }),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_class_declaration_baseline() {
        let parsed = parse_script("class C {} C;").expect("script parsing should succeed");
        match &parsed.statements[0] {
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: Identifier(name),
                initializer: Some(Expr::Call { .. }),
            }) if name == "C" => {}
            _ => panic!("expected lowered class declaration initializer"),
        }
        assert_eq!(
            parsed.statements[1],
            Stmt::Expression(Expr::Identifier(Identifier("C".to_string())))
        );
    }

    #[test]
    fn parses_class_expression_baseline() {
        let parsed = parse_expression("class await {}").expect("parser should succeed");
        assert!(matches!(parsed, Expr::Call { .. }));
    }

    #[test]
    fn parses_class_computed_accessors_baseline() {
        parse_script("class C { get ['a']() { return 1; } set ['a'](v) {} }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_class_computed_method_with_in_in_for_init() {
        parse_script("for (class C { ['x' in empty]() {} }; ; ) break;")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_class_static_super_assignment_baseline() {
        let parsed = parse_script("class C { static m() { super.x = 1; } }")
            .expect("script parsing should succeed");
        match &parsed.statements[0] {
            Stmt::VariableDeclaration(VariableDeclaration {
                initializer: Some(Expr::Call { .. }),
                ..
            }) => {}
            _ => panic!("expected lowered class initializer call expression"),
        }
    }

    #[test]
    fn parses_class_instance_super_member_reference_baseline() {
        parse_script("class C { m() { super.x; } }").expect("script parsing should succeed");
    }

    #[test]
    fn parses_object_method_super_member_reference_baseline() {
        parse_script("({ m() { super.x; } });").expect("script parsing should succeed");
    }

    #[test]
    fn parses_object_accessor_super_member_reference_baseline() {
        parse_script("({ get x() { return super.x; }, set x(v) { super.x = v; } });")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_anonymous_class_expression_with_extends_baseline() {
        parse_script("let C = class extends null {};").expect("script parsing should succeed");
    }

    #[test]
    fn rejects_top_level_super_member_reference_baseline() {
        let err = parse_script("super.x;").expect_err("parser should fail");
        assert_eq!(err.message, "reserved word cannot be identifier reference");
    }

    #[test]
    fn parses_class_static_prototype_computed_member_baseline() {
        parse_script("class C { static ['prototype']() {} }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_async_and_generator_function_forms_baseline() {
        parse_script("async function f() {}").expect("script parsing should succeed");
        parse_script("async function * g() {}").expect("script parsing should succeed");
        parse_script("function * h() {}").expect("script parsing should succeed");
        parse_expression("(async function foo() {}.prototype)")
            .expect("expression parsing should succeed");
    }

    #[test]
    fn parses_generator_yield_statements_baseline() {
        parse_script("function* g() { yield 1; if (true) { yield; } yield ''; }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_generator_yield_star_baseline() {
        parse_script("function* g(iter) { yield* iter; }").expect("script parsing should succeed");
    }

    #[test]
    fn lowers_generator_yield_star_in_function_expression() {
        let parsed = parse_script("var g = function* (iter) { yield* iter; };")
            .expect("script parsing should succeed");
        let debug = format!("{parsed:?}");
        assert!(
            debug.contains("$__qjs_generator_values_"),
            "lowered generator should introduce an internal yield buffer"
        );
        assert!(
            debug.contains("property: \"push\""),
            "lowered generator yield* should push delegated value into the internal buffer"
        );
    }

    #[test]
    fn lowers_generator_yield_statements_into_array_collection() {
        let parsed = parse_script("function* g() { yield 1; yield ''; }")
            .expect("script parsing should succeed");
        let debug = format!("{parsed:?}");
        assert!(
            debug.contains("$__qjs_generator_values_"),
            "lowered generator should introduce an internal yield buffer"
        );
        assert!(
            debug.contains("property: \"push\""),
            "lowered generator yield should push values into the internal buffer"
        );
    }

    #[test]
    fn parses_async_function_await_expression_baseline() {
        parse_script("async function foo() { await new Promise(); }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_await_identifier_in_nested_non_async_function() {
        parse_script("var await; async function foo() { function bar() { await = 1; } bar(); }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_assignment_pattern_array_and_object_baseline() {
        parse_script("([a = thrower()] = iterator); ([target.a] = iterator);")
            .expect("script parsing should succeed");
        parse_script("({ __proto__: x, __proto__: y } = value);")
            .expect("script parsing should succeed");
    }

    #[test]
    fn lowers_array_assignment_pattern_with_default_initializer() {
        let parsed = parse_expression("([a = thrower()] = iterator)")
            .expect("expression parsing should succeed");
        let debug = format!("{parsed:?}");
        assert!(
            debug.contains("thrower"),
            "lowered assignment should preserve default initializer call"
        );
        assert!(
            debug.contains("__forOfIterator") && debug.contains("__forOfStep"),
            "lowered assignment should use for-of iterator helpers"
        );
    }

    #[test]
    fn parses_rest_parameter_binding_pattern_baseline() {
        parse_script("function f(...[]) {} function g(...{}) {}")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_script_with_variable_declarations() {
        let parsed = parse_script("let x = 1; const y = x + 2; x = y * 3; x;")
            .expect("script parsing should succeed");
        let expected = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("x".to_string()),
                    initializer: Some(Expr::Number(1.0)),
                }),
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Const,
                    name: Identifier("y".to_string()),
                    initializer: Some(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                        right: Box::new(Expr::Number(2.0)),
                    }),
                }),
                Stmt::Expression(Expr::Assign {
                    target: Identifier("x".to_string()),
                    value: Box::new(Expr::Binary {
                        op: BinaryOp::Mul,
                        left: Box::new(Expr::Identifier(Identifier("y".to_string()))),
                        right: Box::new(Expr::Number(3.0)),
                    }),
                }),
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_var_declaration() {
        let parsed = parse_script("var x = 1; x;").expect("script parsing should succeed");
        let expected = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Var,
                    name: Identifier("x".to_string()),
                    initializer: Some(Expr::Number(1.0)),
                }),
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_var_declaration_list() {
        let parsed = parse_script("var x, y = 1;").expect("script parsing should succeed");
        let expected = Script {
            statements: vec![Stmt::VariableDeclarations(vec![
                VariableDeclaration {
                    kind: BindingKind::Var,
                    name: Identifier("x".to_string()),
                    initializer: None,
                },
                VariableDeclaration {
                    kind: BindingKind::Var,
                    name: Identifier("y".to_string()),
                    initializer: Some(Expr::Number(1.0)),
                },
            ])],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn allows_line_terminator_as_statement_separator() {
        let parsed =
            parse_script("var x = 1\nx = x + 1\nx").expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 3);
    }

    #[test]
    fn parses_empty_statement() {
        let parsed = parse_script("let x = 1;;x;").expect("script parsing should succeed");
        assert!(matches!(parsed.statements[1], Stmt::Empty));
    }

    #[test]
    fn parses_do_while_statement() {
        let parsed = parse_script("do ; while (false);").expect("script parsing should succeed");
        assert!(matches!(parsed.statements[0], Stmt::DoWhile { .. }));
    }

    #[test]
    fn allows_comment_line_separator_for_asi() {
        let parsed = parse_script("''/*\u{2028}*/''").expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 2);
    }

    #[test]
    fn allows_comment_paragraph_separator_for_asi() {
        let parsed = parse_script("''/*\u{2029}*/''").expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 2);
    }

    #[test]
    fn parses_labeled_statement() {
        let parsed = parse_script("a: 1;").expect("script parsing should succeed");
        assert!(matches!(parsed.statements[0], Stmt::Labeled { .. }));
    }

    #[test]
    fn parses_labeled_break_statement() {
        let parsed =
            parse_script("outer: { break outer; }").expect("script parsing should succeed");
        let body = match &parsed.statements[0] {
            Stmt::Labeled { body, .. } => body,
            _ => panic!("expected labeled statement"),
        };
        let statements = match body.as_ref() {
            Stmt::Block(statements) => statements,
            _ => panic!("expected block body"),
        };
        assert!(matches!(
            statements[0],
            Stmt::BreakLabel(Identifier(ref name)) if name == "outer"
        ));
    }

    #[test]
    fn parses_block_statement_and_shadowing_syntax() {
        let parsed = parse_script("let x = 1; { let x = 2; x = x + 1; }; x;")
            .expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 3);
        assert!(matches!(parsed.statements[1], Stmt::Block(_)));
    }

    #[test]
    fn allows_statement_after_block_without_semicolon() {
        let parsed =
            parse_script("{ let x = 1; } let y = 2; y;").expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 3);
    }

    #[test]
    fn parses_if_else_statement() {
        let parsed =
            parse_script("if (1 < 2) x = 1; else x = 2;").expect("script parsing should succeed");
        let expected = Script {
            statements: vec![Stmt::If {
                condition: Expr::Binary {
                    op: BinaryOp::Less,
                    left: Box::new(Expr::Number(1.0)),
                    right: Box::new(Expr::Number(2.0)),
                },
                consequent: Box::new(Stmt::Expression(Expr::Assign {
                    target: Identifier("x".to_string()),
                    value: Box::new(Expr::Number(1.0)),
                })),
                alternate: Some(Box::new(Stmt::Expression(Expr::Assign {
                    target: Identifier("x".to_string()),
                    value: Box::new(Expr::Number(2.0)),
                }))),
            }],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_while_statement() {
        let parsed = parse_script("let x = 0; while (x < 3) x = x + 1; x;")
            .expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 3);
        assert!(matches!(parsed.statements[1], Stmt::While { .. }));
    }

    #[test]
    fn parses_debugger_statement_baseline() {
        parse_script("while (false) debugger;").expect("script parsing should succeed");
    }

    #[test]
    fn parses_with_statement_baseline() {
        parse_script("with ({}) { 'use strict'; }").expect("script parsing should succeed");
    }

    #[test]
    fn rejects_with_statement_in_strict_mode() {
        let err = parse_script("\"use strict\"; with ({}) {}")
            .expect_err("strict mode with statement should fail");
        assert_eq!(err.message, "with statement not allowed in strict mode");
    }

    #[test]
    fn parses_embedded_let_expression_statement_with_asi() {
        parse_script("if (false) let\n{}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_statement() {
        let parsed = parse_script("for (let i = 0; i < 3; i = i + 1) i;")
            .expect("script parsing should succeed");
        let expected = Script {
            statements: vec![Stmt::For {
                initializer: Some(ForInitializer::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("i".to_string()),
                    initializer: Some(Expr::Number(0.0)),
                })),
                condition: Some(Expr::Binary {
                    op: BinaryOp::Less,
                    left: Box::new(Expr::Identifier(Identifier("i".to_string()))),
                    right: Box::new(Expr::Number(3.0)),
                }),
                update: Some(Expr::Assign {
                    target: Identifier("i".to_string()),
                    value: Box::new(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Identifier(Identifier("i".to_string()))),
                        right: Box::new(Expr::Number(1.0)),
                    }),
                }),
                body: Box::new(Stmt::Expression(Expr::Identifier(Identifier(
                    "i".to_string(),
                )))),
            }],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_for_in_with_let_identifier_baseline() {
        parse_script("for (let in {}) {}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_head_let_expression_forms() {
        parse_script("var let; for (let; ; ) break;").expect("script parsing should succeed");
        parse_script("var let; for (let = 3; ; ) break;").expect("script parsing should succeed");
        parse_script("var let; for ([let][0]; ; ) break;").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_head_let_array_destructuring_declaration() {
        parse_script("for (let [x] = [23]; ; ) { break; }").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_head_let_object_destructuring_declaration() {
        parse_script("for (let {x = init()} = values; ; ) { break; }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_head_async_of_arrow_expression() {
        parse_script("for (async of => {}; i < 10; ++i) { counter = counter + 1; }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_test262_head_lhs_let_shape() {
        let source = r#"
var let;

let = 1;
for ( let; ; )
  break;

let = 2;
for ( let = 3; ; )
  break;

let = 4;
for ( [let][0]; ; )
  break;
"#;
        parse_script(source).expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_in_of_with_embedded_let_asi_baseline() {
        parse_script("for (var x in null) let\n{}").expect("script parsing should succeed");
        parse_script("for (var x of []) let\n{}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_in_expression_initializer() {
        parse_script("for (x in y) {}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_in_const_initializerless_declaration() {
        parse_script("for (const x in { key: 0 }) {}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_of_const_initializerless_declaration_baseline() {
        parse_script("for (const x of [1, 2, 3]) {}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_of_let_object_pattern_with_default_binding() {
        parse_script("for (let { era, aliases = [] } of records) {}")
            .expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_await_of_inside_async_function() {
        parse_script("async function f() { for await (const x of xs) { x; } }")
            .expect("script parsing should succeed");
    }

    #[test]
    fn rejects_for_await_of_outside_async_function() {
        let err = parse_script("for await (const x of xs) {}")
            .expect_err("for-await should fail outside async function");
        assert_eq!(err.message, "for-await is only valid in async functions");
    }

    #[test]
    fn parses_variable_array_destructuring_declaration_baseline() {
        parse_script("const [x = init()] = values;").expect("script parsing should succeed");
    }

    #[test]
    fn parses_variable_object_destructuring_declaration_baseline() {
        parse_script("const {x = init()} = values;").expect("script parsing should succeed");
    }

    #[test]
    fn parses_let_object_destructuring_declaration_baseline() {
        parse_script("let {x = init()} = values;").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_in_array_pattern_heads_baseline() {
        parse_script("for (let [x] in obj) {}").expect("script parsing should succeed");
        parse_script("for (var [x, x] in obj) {}").expect("script parsing should succeed");
        parse_script("for (let [_ = probe = 1] in obj) {}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_for_in_rhs_comma_expression() {
        parse_script("for (x in null, { key: 0 }) {}").expect("script parsing should succeed");
    }

    #[test]
    fn parses_break_and_continue_inside_loop() {
        let parsed =
            parse_script("for (;;) { continue; break; }").expect("script parsing should succeed");
        let body = match &parsed.statements[0] {
            Stmt::For { body, .. } => body,
            _ => panic!("expected for statement"),
        };
        let statements = match body.as_ref() {
            Stmt::Block(statements) => statements,
            _ => panic!("expected block body"),
        };
        assert!(matches!(statements[0], Stmt::Continue));
        assert!(matches!(statements[1], Stmt::Break));
    }

    #[test]
    fn parses_labeled_continue_statement() {
        let parsed = parse_script("outer: for (;;) { continue outer; }")
            .expect("script parsing should succeed");
        let body = match &parsed.statements[0] {
            Stmt::Labeled { body, .. } => body,
            _ => panic!("expected labeled statement"),
        };
        let loop_body = match body.as_ref() {
            Stmt::For { body, .. } => body,
            _ => panic!("expected for statement"),
        };
        let statements = match loop_body.as_ref() {
            Stmt::Block(statements) => statements,
            _ => panic!("expected block body"),
        };
        assert!(matches!(
            statements[0],
            Stmt::ContinueLabel(Identifier(ref name)) if name == "outer"
        ));
    }

    #[test]
    fn parses_switch_statement() {
        let parsed = parse_script("switch (x) { case 1: y = 2; break; default: y = 3; }")
            .expect("script parsing should succeed");
        let expected = Script {
            statements: vec![Stmt::Switch {
                discriminant: Expr::Identifier(Identifier("x".to_string())),
                cases: vec![
                    SwitchCase {
                        test: Some(Expr::Number(1.0)),
                        consequent: vec![
                            Stmt::Expression(Expr::Assign {
                                target: Identifier("y".to_string()),
                                value: Box::new(Expr::Number(2.0)),
                            }),
                            Stmt::Break,
                        ],
                    },
                    SwitchCase {
                        test: None,
                        consequent: vec![Stmt::Expression(Expr::Assign {
                            target: Identifier("y".to_string()),
                            value: Box::new(Expr::Number(3.0)),
                        })],
                    },
                ],
            }],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn allows_break_inside_switch() {
        let parsed =
            parse_script("switch (1) { case 1: break; }").expect("script parsing should succeed");
        assert!(matches!(parsed.statements[0], Stmt::Switch { .. }));
    }

    #[test]
    fn parses_throw_statement() {
        let parsed = parse_script("throw 42;").expect("script parsing should succeed");
        let expected = Script {
            statements: vec![Stmt::Throw(Expr::Number(42.0))],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_try_catch_finally_statement() {
        let parsed = parse_script("try { throw 1; } catch (e) { e; } finally { 0; }")
            .expect("script parsing should succeed");
        let expected = Script {
            statements: vec![Stmt::Try {
                try_block: vec![Stmt::Throw(Expr::Number(1.0))],
                catch_param: Some(Identifier("e".to_string())),
                catch_block: Some(vec![Stmt::Expression(Expr::Identifier(Identifier(
                    "e".to_string(),
                )))]),
                finally_block: Some(vec![Stmt::Expression(Expr::Number(0.0))]),
            }],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_try_catch_with_array_pattern_parameter() {
        let parsed = parse_script("try { throw ['inside']; } catch ([x, _ = 1]) { x; _; }")
            .expect("script parsing should succeed");
        let Stmt::Try {
            catch_param,
            catch_block,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected try statement");
        };
        let Some(Identifier(param_name)) = catch_param else {
            panic!("expected synthesized catch parameter");
        };
        assert!(param_name.starts_with("$__catch_param_"));

        let catch_block = catch_block.as_ref().expect("expected catch block");
        assert!(matches!(
            catch_block.first(),
            Some(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: Identifier(name),
                ..
            })) if name == "x"
        ));
        assert!(matches!(
            catch_block.get(1),
            Some(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: Identifier(name),
                ..
            })) if name == "_"
        ));
    }

    #[test]
    fn parses_try_catch_with_object_pattern_parameter() {
        let parsed = parse_script("try { throw { f: 1 }; } catch ({ f }) { f; }")
            .expect("script parsing should succeed");
        let Stmt::Try {
            catch_param,
            catch_block,
            ..
        } = &parsed.statements[0]
        else {
            panic!("expected try statement");
        };
        let Some(Identifier(param_name)) = catch_param else {
            panic!("expected synthesized catch parameter");
        };
        assert!(param_name.starts_with("$__catch_param_"));

        let catch_block = catch_block.as_ref().expect("expected catch block");
        assert!(matches!(
            catch_block.first(),
            Some(Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: Identifier(name),
                ..
            })) if name == "f"
        ));
    }

    #[test]
    fn rejects_return_outside_function() {
        let err = parse_script("return 1;").expect_err("parser should fail");
        assert_eq!(err.message, "return outside function");
    }

    #[test]
    fn rejects_break_outside_loop() {
        let err = parse_script("break;").expect_err("parser should fail");
        assert_eq!(err.message, "break outside loop or switch");
    }

    #[test]
    fn rejects_break_with_undefined_label() {
        let err = parse_script("break outer;").expect_err("parser should fail");
        assert_eq!(err.message, "undefined label: outer");
    }

    #[test]
    fn rejects_break_to_outer_label_inside_function() {
        let err = parse_script("outer: { function f() { break outer; } }")
            .expect_err("parser should fail");
        assert_eq!(err.message, "undefined label: outer");
    }

    #[test]
    fn rejects_continue_to_non_iteration_label() {
        let err = parse_script("outer: { continue outer; }").expect_err("parser should fail");
        assert_eq!(err.message, "continue target must be iteration statement");
    }

    #[test]
    fn rejects_duplicate_label() {
        let err = parse_script("outer: { outer: 1; }").expect_err("parser should fail");
        assert_eq!(err.message, "duplicate label: outer");
    }

    #[test]
    fn rejects_continue_outside_loop() {
        let err = parse_script("continue;").expect_err("parser should fail");
        assert_eq!(err.message, "continue outside loop");
    }

    #[test]
    fn rejects_try_without_catch_or_finally() {
        let err = parse_script("try { 1; }").expect_err("parser should fail");
        assert_eq!(err.message, "try requires catch or finally");
    }

    #[test]
    fn rejects_throw_without_expression() {
        let err = parse_script("throw;").expect_err("parser should fail");
        assert_eq!(err.message, "throw requires expression");
    }

    #[test]
    fn allows_function_declaration_in_embedded_statement_annex_b() {
        assert!(parse_script("while (1) function f() {}").is_ok());
    }

    #[test]
    fn lowers_embedded_function_declaration_to_block_annex_b() {
        let parsed =
            parse_script("if (true) function f() {} else ;").expect("parser should succeed");
        let Script { statements } = parsed;
        let [
            Stmt::If {
                consequent,
                alternate,
                ..
            },
        ] = statements.as_slice()
        else {
            panic!("expected single if statement");
        };
        let Stmt::Block(consequent_body) = consequent.as_ref() else {
            panic!("expected embedded declaration to be wrapped in block");
        };
        assert!(matches!(
            consequent_body.as_slice(),
            [Stmt::FunctionDeclaration(FunctionDeclaration {
                name: Identifier(name),
                ..
            })] if name == "f"
        ));
        assert!(matches!(alternate.as_deref(), Some(Stmt::Empty)));
    }

    #[test]
    fn rejects_else_after_if_block_with_extra_semicolon() {
        assert!(parse_script("if (false) {}; else {}").is_err());
    }

    #[test]
    fn rejects_do_while_with_semicolon_after_block_body() {
        assert!(parse_script("do {}; while (false)").is_err());
    }

    #[test]
    fn rejects_duplicate_default_in_switch() {
        let err =
            parse_script("switch (x) { default: 1; default: 2; }").expect_err("parser should fail");
        assert_eq!(err.message, "duplicate default in switch");
    }

    #[test]
    fn rejects_const_without_initializer() {
        let err = parse_script("const x;").expect_err("parser should fail");
        assert_eq!(err.message, "const declaration requires an initializer");
    }

    #[test]
    fn rejects_duplicate_lexical_declaration() {
        let err = parse_script("{ let x = 1; const x = 2; }").expect_err("parser should fail");
        assert_eq!(err.message, "duplicate lexical declaration: x");
    }

    #[test]
    fn rejects_lexical_var_redeclaration_in_block() {
        let err = parse_script("{ const f = 0; var f; }").expect_err("parser should fail");
        assert_eq!(
            err.message,
            "lexical declaration conflicts with var/function declaration: f"
        );
    }

    #[test]
    fn rejects_lexical_function_redeclaration_in_block() {
        let err = parse_script("{ let f; function f() {} }").expect_err("parser should fail");
        assert_eq!(err.message, "duplicate lexical declaration: f");
    }

    #[test]
    fn rejects_function_var_redeclaration_in_block() {
        let err = parse_script("{ function f() {} var f; }").expect_err("parser should fail");
        assert_eq!(
            err.message,
            "lexical declaration conflicts with var/function declaration: f"
        );
    }

    #[test]
    fn rejects_var_function_redeclaration_in_block() {
        let err = parse_script("{ var f; function f() {} }").expect_err("parser should fail");
        assert_eq!(
            err.message,
            "lexical declaration conflicts with var/function declaration: f"
        );
    }

    #[test]
    fn rejects_nested_block_var_conflict_with_block_function() {
        let err = parse_script("{ function f() {} { var f; } }").expect_err("parser should fail");
        assert_eq!(
            err.message,
            "lexical declaration conflicts with var/function declaration: f"
        );
    }

    #[test]
    fn allows_var_redeclaration() {
        parse_script("var f; var f;").expect("parser should succeed");
    }

    #[test]
    fn allows_let_identifier_in_non_strict_var_and_reference_positions() {
        parse_script("var let = 1; var object = {let};").expect("parser should succeed");
        let parsed = parse_expression("let").expect("parser should succeed");
        assert_eq!(parsed, Expr::Identifier(Identifier("let".to_string())));
    }

    #[test]
    fn allows_future_reserved_words_as_identifiers_in_non_strict_mode() {
        parse_script(
            "var implements = 1; var interface = 2; var package = 3; var private = 4; var protected = 5; var public = 6; var static = 7;",
        )
        .expect("parser should succeed");
        let parsed = parse_expression("implements + static").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Identifier(Identifier("implements".to_string()))),
            right: Box::new(Expr::Identifier(Identifier("static".to_string()))),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn rejects_future_reserved_binding_in_strict_mode() {
        let err = parse_script("'use strict'; var public = 1;").expect_err("parser should fail");
        assert_eq!(err.message, "reserved word cannot be binding identifier");
    }

    #[test]
    fn rejects_eval_binding_in_strict_mode() {
        let err = parse_script("'use strict'; var eval = 1;").expect_err("parser should fail");
        assert_eq!(err.message, "invalid binding identifier in strict mode");
    }

    #[test]
    fn rejects_arguments_parameter_in_strict_mode() {
        let err =
            parse_script("'use strict'; function f(arguments) {}").expect_err("parser should fail");
        assert_eq!(err.message, "invalid binding identifier in strict mode");
    }

    #[test]
    fn rejects_future_reserved_reference_in_strict_mode() {
        let err = parse_script("function f() { 'use strict'; public = 1; }")
            .expect_err("parser should fail");
        assert_eq!(err.message, "reserved word cannot be identifier reference");
    }

    #[test]
    fn rejects_eval_assignment_target_in_strict_mode() {
        let err = parse_script("'use strict'; eval = 1;").expect_err("parser should fail");
        assert_eq!(err.message, "invalid lvalue in strict mode");
    }

    #[test]
    fn allows_eval_identifier_reference_in_strict_mode() {
        parse_script("'use strict'; eval;").expect("parser should succeed");
    }

    #[test]
    fn rejects_duplicate_parameters_in_strict_mode() {
        let err =
            parse_script("'use strict'; function f(a, a) {}").expect_err("parser should fail");
        assert_eq!(err.message, "duplicate parameter name in strict mode");
    }

    #[test]
    fn treats_escaped_let_as_identifier_not_keyword() {
        parse_script("l\\u0065t\na;\nvar a;").expect("script parsing should succeed");
    }

    #[test]
    fn allows_function_var_redeclaration() {
        parse_script("function f() {} var f;").expect("parser should succeed");
    }

    #[test]
    fn allows_var_function_redeclaration_at_script_scope() {
        parse_script("var f; function f() {}").expect("parser should succeed");
    }

    #[test]
    fn rejects_switch_case_lexical_var_conflict() {
        let err = parse_script("switch (0) { case 0: let x = 1; case 1: var x; }")
            .expect_err("parser should fail");
        assert_eq!(
            err.message,
            "lexical declaration conflicts with var/function declaration: x"
        );
    }

    #[test]
    fn rejects_catch_parameter_lexical_conflict() {
        let err =
            parse_script("try { 1; } catch (e) { let e = 1; }").expect_err("parser should fail");
        assert_eq!(
            err.message,
            "catch parameter conflicts with lexical declaration: e"
        );
    }

    #[test]
    fn rejects_catch_array_parameter_lexical_conflict() {
        let err =
            parse_script("try { 1; } catch ([x]) { let x = 1; }").expect_err("parser should fail");
        assert_eq!(err.message, "duplicate lexical declaration: x");
    }

    #[test]
    fn rejects_invalid_assignment_target() {
        let err = parse_expression("(x + 1) = 2").expect_err("parser should fail");
        assert_eq!(err.message, "invalid assignment target");
    }

    #[test]
    fn rejects_reserved_word_identifier_reference() {
        let err = parse_script("case = 1;").expect_err("parser should fail");
        assert_eq!(err.message, "reserved word cannot be identifier reference");
    }

    #[test]
    fn rejects_assignment_to_this_expression() {
        let err = parse_script("this = 1;").expect_err("parser should fail");
        assert_eq!(err.message, "invalid assignment target");
    }

    #[test]
    fn rejects_increment_operator_without_support() {
        let err = parse_script("++this;").expect_err("parser should fail");
        assert_eq!(err.message, "invalid update target");
    }

    #[test]
    fn rejects_reserved_word_binding_identifier() {
        let err = parse_script("var break = 1;").expect_err("parser should fail");
        assert_eq!(err.message, "expected binding name");
    }

    #[test]
    fn rejects_reserved_word_shorthand_property() {
        let err = parse_script("({ false });").expect_err("parser should fail");
        assert_eq!(err.message, "reserved word cannot be identifier reference");
    }

    #[test]
    fn rejects_missing_separator_in_if_consequent() {
        let err = parse_script("if (1) x = 1 y = 2;").expect_err("parser should fail");
        assert_eq!(err.message, "expected ';' between statements");
    }

    #[test]
    fn rejects_trailing_tokens() {
        let err = parse_expression("1 2").expect_err("parser should fail");
        assert_eq!(err.message, "unexpected trailing input");
    }

    #[test]
    fn rejects_expression_nesting_too_deep() {
        let mut source = String::new();
        for _ in 0..200 {
            source.push('(');
        }
        source.push('1');
        for _ in 0..200 {
            source.push(')');
        }
        let err = parse_expression(&source).expect_err("parser should fail");
        assert_eq!(err.message, "expression nesting too deep");
    }
}
