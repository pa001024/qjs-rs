#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use ast::{
    BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier, ObjectProperty,
    ObjectPropertyKey, Script, Stmt, SwitchCase, UnaryOp, VariableDeclaration,
};
use lexer::{Token, TokenKind, lex};

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
    let expr = parser.parse_expression_inner()?;
    parser.expect_eof()?;
    Ok(expr)
}

pub fn parse_script(source: &str) -> Result<Script, ParseError> {
    let tokens = lex(source).map_err(|err| ParseError {
        message: err.message,
        position: err.position,
    })?;
    let mut parser = Parser::new(tokens, source);
    let statements = parser.parse_statement_list(None)?;
    validate_early_errors(&statements)?;
    parser.expect_eof()?;
    Ok(Script { statements })
}

fn validate_early_errors(statements: &[Stmt]) -> Result<(), ParseError> {
    validate_statement_list_early_errors(statements, StatementListKind::ScriptOrFunction)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementListKind {
    ScriptOrFunction,
    BlockLike,
}

fn validate_statement_list_early_errors(
    statements: &[Stmt],
    kind: StatementListKind,
) -> Result<(), ParseError> {
    let mut lexical_names = BTreeSet::new();
    for statement in statements {
        collect_direct_lexical_names(statement, &mut lexical_names, kind)?;
    }

    let mut var_declared_names = BTreeSet::new();
    for statement in statements {
        collect_var_declared_names(statement, &mut var_declared_names, kind);
    }

    if let Some(name) = lexical_names
        .iter()
        .find(|candidate| var_declared_names.contains(*candidate))
    {
        return Err(ParseError {
            message: format!("lexical declaration conflicts with var/function declaration: {name}"),
            position: 0,
        });
    }

    for statement in statements {
        validate_nested_statement_early_errors(statement)?;
    }

    Ok(())
}

fn validate_nested_statement_early_errors(statement: &Stmt) -> Result<(), ParseError> {
    match statement {
        Stmt::Block(statements) => {
            validate_statement_list_early_errors(statements, StatementListKind::BlockLike)
        }
        Stmt::FunctionDeclaration(declaration) => validate_statement_list_early_errors(
            &declaration.body,
            StatementListKind::ScriptOrFunction,
        ),
        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            validate_nested_statement_early_errors(consequent)?;
            if let Some(alternate) = alternate {
                validate_nested_statement_early_errors(alternate)?;
            }
            Ok(())
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::Labeled { body, .. } => validate_nested_statement_early_errors(body),
        Stmt::Switch { cases, .. } => validate_switch_case_early_errors(cases),
        Stmt::Try {
            try_block,
            catch_param,
            catch_block,
            finally_block,
        } => {
            validate_statement_list_early_errors(try_block, StatementListKind::BlockLike)?;
            if let Some(catch_block) = catch_block {
                validate_catch_block_early_errors(catch_param.as_ref(), catch_block)?;
            }
            if let Some(finally_block) = finally_block {
                validate_statement_list_early_errors(finally_block, StatementListKind::BlockLike)?;
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

fn validate_switch_case_early_errors(cases: &[SwitchCase]) -> Result<(), ParseError> {
    let mut lexical_names = BTreeSet::new();
    for case in cases {
        for statement in &case.consequent {
            collect_direct_lexical_names(
                statement,
                &mut lexical_names,
                StatementListKind::BlockLike,
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
        .iter()
        .find(|candidate| var_declared_names.contains(*candidate))
    {
        return Err(ParseError {
            message: format!("lexical declaration conflicts with var/function declaration: {name}"),
            position: 0,
        });
    }

    for case in cases {
        for statement in &case.consequent {
            validate_nested_statement_early_errors(statement)?;
        }
    }

    Ok(())
}

fn validate_catch_block_early_errors(
    catch_param: Option<&Identifier>,
    catch_block: &[Stmt],
) -> Result<(), ParseError> {
    validate_statement_list_early_errors(catch_block, StatementListKind::BlockLike)?;

    if let Some(catch_param) = catch_param {
        let mut lexical_names = BTreeSet::new();
        for statement in catch_block {
            collect_direct_lexical_names(
                statement,
                &mut lexical_names,
                StatementListKind::BlockLike,
            )?;
        }
        if lexical_names.contains(&catch_param.0) {
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
    lexical_names: &mut BTreeSet<String>,
    kind: StatementListKind,
) -> Result<(), ParseError> {
    match statement {
        Stmt::VariableDeclaration(declaration) => {
            add_lexical_name_if_needed(lexical_names, declaration)?;
        }
        Stmt::VariableDeclarations(declarations) => {
            for declaration in declarations {
                add_lexical_name_if_needed(lexical_names, declaration)?;
            }
        }
        Stmt::FunctionDeclaration(declaration) => {
            if kind == StatementListKind::BlockLike {
                add_lexical_name(lexical_names, &declaration.name.0)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn add_lexical_name_if_needed(
    lexical_names: &mut BTreeSet<String>,
    declaration: &VariableDeclaration,
) -> Result<(), ParseError> {
    if !matches!(declaration.kind, BindingKind::Let | BindingKind::Const) {
        return Ok(());
    }
    add_lexical_name(lexical_names, &declaration.name.0)
}

fn add_lexical_name(lexical_names: &mut BTreeSet<String>, name: &str) -> Result<(), ParseError> {
    if !lexical_names.insert(name.to_string()) {
        return Err(ParseError {
            message: format!("duplicate lexical declaration: {name}"),
            position: 0,
        });
    }
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
            collect_var_declared_names(consequent, var_declared_names, kind);
            if let Some(alternate) = alternate {
                collect_var_declared_names(alternate, var_declared_names, kind);
            }
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Labeled { body, .. } => {
            collect_var_declared_names(body, var_declared_names, kind);
        }
        Stmt::For {
            initializer, body, ..
        } => {
            if let Some(initializer) = initializer {
                collect_var_declared_names_from_for_initializer(initializer, var_declared_names);
            }
            collect_var_declared_names(body, var_declared_names, kind);
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
        | Stmt::Continue => {}
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

#[derive(Debug)]
struct Parser {
    tokens: Vec<Token>,
    source: String,
    pos: usize,
    expression_depth: usize,
    function_depth: usize,
    loop_depth: usize,
    breakable_depth: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>, source: &str) -> Self {
        Self {
            tokens,
            source: source.to_string(),
            pos: 0,
            expression_depth: 0,
            function_depth: 0,
            loop_depth: 0,
            breakable_depth: 0,
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
            return self.parse_function_declaration_statement();
        }
        if self.matches_keyword("function") {
            return self.parse_function_declaration_statement();
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
        if self.matches_keyword("let") {
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
        let expr = self.parse_expression_inner()?;
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

    fn parse_function_declaration_statement(&mut self) -> Result<Stmt, ParseError> {
        let _is_generator = self.matches(&TokenKind::Star);
        let name = Identifier(self.expect_binding_identifier("expected function name")?);
        self.expect(TokenKind::LParen, "expected '(' after function name")?;
        let params = self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        self.function_depth += 1;
        let body = self.parse_block_body(
            "expected '{' before function body",
            "expected '}' after function body",
        );
        self.function_depth = self.function_depth.saturating_sub(1);
        let body = body?;

        Ok(Stmt::FunctionDeclaration(FunctionDeclaration {
            name,
            params,
            body,
        }))
    }

    fn parse_class_declaration_statement(&mut self) -> Result<Stmt, ParseError> {
        let name = Identifier(self.expect_binding_identifier("expected class name")?);
        self.parse_class_tail()?;
        Ok(Stmt::VariableDeclaration(VariableDeclaration {
            kind: BindingKind::Let,
            name,
            initializer: Some(Expr::ObjectLiteral(vec![])),
        }))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'if'")?;
        let condition = self.parse_expression_inner()?;
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
        let condition = self.parse_expression_inner()?;
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
        let condition = self.parse_expression_inner()?;
        self.expect(TokenKind::RParen, "expected ')' after do-while condition")?;
        let _ = self.matches(&TokenKind::Semicolon);
        Ok(Stmt::DoWhile {
            body: Box::new(body),
            condition,
        })
    }

    fn parse_for_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'for'")?;

        let initializer = if self.check(&TokenKind::Semicolon) {
            None
        } else if self.matches_keyword("let") {
            if self.check_keyword("in") || self.check_keyword("of") {
                Some(ForInitializer::Expression(Expr::Identifier(Identifier(
                    "let".to_string(),
                ))))
            } else {
                let declaration = self.parse_variable_declaration(BindingKind::Let)?;
                let declaration = match declaration {
                    Stmt::VariableDeclaration(declaration) => {
                        ForInitializer::VariableDeclaration(declaration)
                    }
                    Stmt::VariableDeclarations(declarations) => {
                        ForInitializer::VariableDeclarations(declarations)
                    }
                    _ => {
                        return Err(ParseError {
                            message: "invalid for initializer".to_string(),
                            position: self.current_position(),
                        });
                    }
                };
                Some(declaration)
            }
        } else if self.matches_keyword("const") {
            let declaration = self.parse_variable_declaration(BindingKind::Const)?;
            let declaration = match declaration {
                Stmt::VariableDeclaration(declaration) => {
                    ForInitializer::VariableDeclaration(declaration)
                }
                Stmt::VariableDeclarations(declarations) => {
                    ForInitializer::VariableDeclarations(declarations)
                }
                _ => {
                    return Err(ParseError {
                        message: "invalid for initializer".to_string(),
                        position: self.current_position(),
                    });
                }
            };
            Some(declaration)
        } else if self.matches_keyword("var") {
            let declaration = self.parse_variable_declaration(BindingKind::Var)?;
            let declaration = match declaration {
                Stmt::VariableDeclaration(declaration) => {
                    ForInitializer::VariableDeclaration(declaration)
                }
                Stmt::VariableDeclarations(declarations) => {
                    ForInitializer::VariableDeclarations(declarations)
                }
                _ => {
                    return Err(ParseError {
                        message: "invalid for initializer".to_string(),
                        position: self.current_position(),
                    });
                }
            };
            Some(declaration)
        } else {
            Some(ForInitializer::Expression(self.parse_expression_inner()?))
        };
        if self.matches_keyword("in") || self.matches_keyword("of") {
            let _ = self.parse_expression_inner()?;
            self.expect(TokenKind::RParen, "expected ')' after for-in/of clauses")?;

            self.loop_depth += 1;
            self.breakable_depth += 1;
            let body = self.parse_embedded_statement(false);
            self.loop_depth = self.loop_depth.saturating_sub(1);
            self.breakable_depth = self.breakable_depth.saturating_sub(1);
            let body = body?;

            // Baseline: parse/compile `for-in` and `for-of` shape as non-iterating loops.
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
            Some(self.parse_expression_inner()?)
        };
        self.expect(TokenKind::Semicolon, "expected ';' after for condition")?;

        let update = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(self.parse_expression_inner()?)
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

    fn parse_with_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'with'")?;
        let _ = self.parse_expression_inner()?;
        self.expect(TokenKind::RParen, "expected ')' after with object")?;
        self.parse_embedded_statement(false)
    }

    fn parse_switch_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'switch'")?;
        let discriminant = self.parse_expression_inner()?;
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
                let test = self.parse_expression_inner()?;
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
        if self.matches_keyword("catch") {
            self.expect(TokenKind::LParen, "expected '(' after 'catch'")?;
            catch_param = Some(Identifier(
                self.expect_binding_identifier("expected catch binding identifier")?,
            ));
            self.expect(TokenKind::RParen, "expected ')' after catch binding")?;
            catch_block = Some(self.parse_block_body(
                "expected '{' before catch block",
                "expected '}' after catch block",
            )?);
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
        let expr = self.parse_expression_inner()?;
        Ok(Stmt::Throw(expr))
    }

    fn parse_labeled_statement(&mut self) -> Result<Stmt, ParseError> {
        let label = Identifier(self.expect_binding_identifier("expected label identifier")?);
        self.expect(TokenKind::Colon, "expected ':' after label")?;
        let body = self.parse_embedded_statement(false)?;
        Ok(Stmt::Labeled {
            label,
            body: Box::new(body),
        })
    }

    fn parse_break_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.breakable_depth == 0 {
            return Err(ParseError {
                message: "break outside loop or switch".to_string(),
                position: self.previous_position(),
            });
        }
        Ok(Stmt::Break)
    }

    fn parse_continue_statement(&mut self) -> Result<Stmt, ParseError> {
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
        let statement = if self.check_keyword("let") && !self.check_next(&TokenKind::Colon) {
            Stmt::Expression(self.parse_expression_inner()?)
        } else {
            self.parse_statement()?
        };
        if matches!(statement, Stmt::FunctionDeclaration(_)) {
            return Err(
                self.error_current("function declaration not allowed in statement position")
            );
        }
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
            let expr = self.parse_expression_inner()?;
            Ok(Stmt::Return(Some(expr)))
        } else {
            Ok(Stmt::Return(None))
        }
    }

    fn parse_variable_declaration(&mut self, kind: BindingKind) -> Result<Stmt, ParseError> {
        let mut declarations = Vec::new();
        loop {
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

            if kind == BindingKind::Const && initializer.is_none() {
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

    fn parse_assignment(&mut self) -> Result<Expr, ParseError> {
        if let Some(arrow_function) = self.try_parse_arrow_function()? {
            return Ok(arrow_function);
        }

        let left = self.parse_logical_or()?;
        if self.matches(&TokenKind::Equal) {
            let assignment_position = self.previous_position();
            let value = self.parse_assignment()?;
            match left {
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
        } else {
            Ok(left)
        }
    }

    fn try_parse_arrow_function(&mut self) -> Result<Option<Expr>, ParseError> {
        let saved_pos = self.pos;

        let params = if self.matches(&TokenKind::LParen) {
            let mut params = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    let Some(Token {
                        kind: TokenKind::Identifier(name),
                        ..
                    }) = self.current()
                    else {
                        self.pos = saved_pos;
                        return Ok(None);
                    };
                    params.push(Identifier(name.clone()));
                    self.advance();

                    if self.matches(&TokenKind::Comma) {
                        if self.check(&TokenKind::RParen) {
                            break;
                        }
                        continue;
                    }
                    break;
                }
            }

            if !self.matches(&TokenKind::RParen) {
                self.pos = saved_pos;
                return Ok(None);
            }
            params
        } else if self.check_identifier()
            && self.check_next(&TokenKind::Equal)
            && self.check_nth(2, &TokenKind::Greater)
        {
            vec![Identifier(
                self.expect_binding_identifier("expected parameter name")?,
            )]
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

        let body = if self.check(&TokenKind::LBrace) {
            self.function_depth += 1;
            let statements = self.parse_block_body(
                "expected '{' before function body",
                "expected '}' after function body",
            );
            self.function_depth = self.function_depth.saturating_sub(1);
            statements?
        } else {
            vec![Stmt::Return(Some(self.parse_assignment()?))]
        };

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

    fn parse_logical_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_comparison()?;
        while self.matches(&TokenKind::AndAnd) {
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                op: BinaryOp::LogicalAnd,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_additive()?;
        loop {
            let op = if self.matches(&TokenKind::EqualEqualEqual) {
                BinaryOp::StrictEqual
            } else if self.matches(&TokenKind::BangEqualEqual) {
                BinaryOp::StrictNotEqual
            } else if self.matches(&TokenKind::EqualEqual) {
                BinaryOp::Equal
            } else if self.matches(&TokenKind::BangEqual) {
                BinaryOp::NotEqual
            } else if self.matches(&TokenKind::Less) {
                BinaryOp::Less
            } else if self.matches(&TokenKind::LessEqual) {
                BinaryOp::LessEqual
            } else if self.matches(&TokenKind::Greater) {
                BinaryOp::Greater
            } else if self.matches(&TokenKind::GreaterEqual) {
                BinaryOp::GreaterEqual
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
        if self.matches_keyword("new") {
            return self.parse_new_expression();
        }
        self.parse_postfix()
    }

    fn parse_prefix_update_expression(&mut self, increment: bool) -> Result<Expr, ParseError> {
        let target = self.parse_unary()?;
        self.rewrite_update_target(target, increment)
    }

    fn rewrite_update_target(&self, target: Expr, increment: bool) -> Result<Expr, ParseError> {
        let op = if increment {
            BinaryOp::Add
        } else {
            BinaryOp::Sub
        };
        match target {
            Expr::Identifier(identifier) => Ok(Expr::Assign {
                target: identifier.clone(),
                value: Box::new(Expr::Binary {
                    op,
                    left: Box::new(Expr::Identifier(identifier)),
                    right: Box::new(Expr::Number(1.0)),
                }),
            }),
            Expr::Member { object, property } => Ok(Expr::AssignMember {
                object: object.clone(),
                property: property.clone(),
                value: Box::new(Expr::Binary {
                    op,
                    left: Box::new(Expr::Member { object, property }),
                    right: Box::new(Expr::Number(1.0)),
                }),
            }),
            Expr::MemberComputed { object, property } => Ok(Expr::AssignMemberComputed {
                object: object.clone(),
                property: property.clone(),
                value: Box::new(Expr::Binary {
                    op,
                    left: Box::new(Expr::MemberComputed { object, property }),
                    right: Box::new(Expr::Number(1.0)),
                }),
            }),
            _ => Err(ParseError {
                message: "invalid update target".to_string(),
                position: self.current_position(),
            }),
        }
    }

    fn parse_new_expression(&mut self) -> Result<Expr, ParseError> {
        let mut callee = self.parse_primary()?;
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
            break;
        }
        if self.check(&TokenKind::PlusPlus) {
            if self.has_line_terminator_between_prev_and_current() {
                return Ok(expr);
            }
            self.advance();
            return self.rewrite_update_target(expr, true);
        }
        if self.check(&TokenKind::MinusMinus) {
            if self.has_line_terminator_between_prev_and_current() {
                return Ok(expr);
            }
            self.advance();
            return self.rewrite_update_target(expr, false);
        }
        Ok(expr)
    }

    fn parse_argument_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            let _is_spread = self.matches(&TokenKind::Ellipsis);
            // Baseline parser accepts spread syntax shape first; runtime treats as plain arg.
            args.push(self.parse_expression_inner()?);
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

    fn parse_parameter_list(&mut self) -> Result<Vec<Identifier>, ParseError> {
        let mut params = Vec::new();
        let mut synthetic_index = 0usize;
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let is_rest = self.matches(&TokenKind::Ellipsis);
            let name = if self.check_identifier() {
                self.expect_binding_identifier("expected parameter name")?
            } else {
                self.consume_parameter_pattern("expected parameter name")?;
                let generated = format!("$param_{synthetic_index}");
                synthetic_index += 1;
                generated
            };
            if self.matches(&TokenKind::Equal) {
                let _ = self.parse_expression_inner()?;
            }
            params.push(Identifier(name));
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
        Ok(params)
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

    fn parse_function_expression_after_keyword(&mut self) -> Result<Expr, ParseError> {
        let _is_generator = self.matches(&TokenKind::Star);
        let name = if self.check_identifier() {
            Some(Identifier(
                self.expect_binding_identifier("expected function name")?,
            ))
        } else {
            None
        };
        self.expect(TokenKind::LParen, "expected '(' after 'function'")?;
        let params = self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        self.function_depth += 1;
        let body = self.parse_block_body(
            "expected '{' before function body",
            "expected '}' after function body",
        );
        self.function_depth = self.function_depth.saturating_sub(1);
        let body = body?;

        Ok(Expr::Function { name, params, body })
    }

    fn parse_class_expression_after_keyword(&mut self) -> Result<Expr, ParseError> {
        if self.check_identifier() {
            let _ = self.expect_binding_identifier("expected class name")?;
        }
        self.parse_class_tail()?;
        Ok(Expr::ObjectLiteral(vec![]))
    }

    fn parse_class_tail(&mut self) -> Result<(), ParseError> {
        if self.matches_keyword("extends") {
            let _ = self.parse_expression_inner()?;
        }
        self.consume_balanced_brace_block(
            "expected '{' before class body",
            "expected '}' after class body",
        )
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
                self.advance();
                Ok(Expr::String(value))
            }
            TokenKind::Identifier(name) => {
                if self.identifier_token_matches_keyword(&token, "async")
                    && self.check_next_keyword("function")
                    && !self.has_line_terminator_between_tokens(self.pos, self.pos + 1)
                {
                    self.advance();
                    self.advance();
                    return self.parse_function_expression_after_keyword();
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
                        self.parse_function_expression_after_keyword()
                    }
                    "class" if self.identifier_token_matches_keyword(&token, "class") => {
                        self.parse_class_expression_after_keyword()
                    }
                    _ if self.identifier_token_is_raw_name(&token, &name)
                        && is_forbidden_identifier_reference(&name) =>
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
                let mut expr = self.parse_expression_inner()?;
                while self.matches(&TokenKind::Comma) {
                    expr = self.parse_expression_inner()?;
                }
                self.expect(TokenKind::RParen, "expected ')' after expression")?;
                Ok(expr)
            }
            TokenKind::Slash => self.parse_regex_literal(),
            TokenKind::LBrace => self.parse_object_literal(),
            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::Plus
            | TokenKind::PlusPlus
            | TokenKind::Minus
            | TokenKind::MinusMinus
            | TokenKind::Star
            | TokenKind::Bang
            | TokenKind::Equal
            | TokenKind::EqualEqual
            | TokenKind::EqualEqualEqual
            | TokenKind::BangEqual
            | TokenKind::BangEqualEqual
            | TokenKind::Less
            | TokenKind::LessEqual
            | TokenKind::Greater
            | TokenKind::GreaterEqual
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::Ellipsis
            | TokenKind::Dot
            | TokenKind::Comma
            | TokenKind::Colon => Err(ParseError {
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

    fn parse_object_literal(&mut self) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LBrace, "expected '{' before object literal")?;
        let mut properties = Vec::new();
        if self.matches(&TokenKind::RBrace) {
            return Ok(Expr::ObjectLiteral(properties));
        }
        loop {
            let key = self.parse_object_property_key()?;
            if let Some((accessor_key, accessor_value)) = self.try_parse_object_accessor(&key)? {
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
            let value = if self.matches(&TokenKind::Colon) {
                self.parse_expression_inner()?
            } else if self.check(&TokenKind::LParen) {
                self.parse_object_method_value()?
            } else {
                match &key {
                    ObjectPropertyKey::Static(name) => {
                        if is_forbidden_identifier_reference(name) {
                            return Err(ParseError {
                                message: "reserved word cannot be identifier reference".to_string(),
                                position: self.previous_position(),
                            });
                        }
                        Expr::Identifier(Identifier(name.clone()))
                    }
                    ObjectPropertyKey::Computed(_) => {
                        return Err(self.error_current(
                            "expected ':' after computed property name in object literal",
                        ));
                    }
                    ObjectPropertyKey::AccessorGet(_) | ObjectPropertyKey::AccessorSet(_) => {
                        return Err(self.error_current("unexpected accessor in object literal"));
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
        if !self.check_identifier() || !self.check_next(&TokenKind::LParen) {
            return Ok(None);
        }
        let accessor_name =
            self.expect_identifier_name("expected property name in object literal")?;
        self.expect(TokenKind::LParen, "expected '(' after accessor name")?;
        let params = self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        self.function_depth += 1;
        let body = self.parse_block_body(
            "expected '{' before function body",
            "expected '}' after function body",
        );
        self.function_depth = self.function_depth.saturating_sub(1);
        let body = body?;

        let accessor_key = if accessor_kind == "get" {
            ObjectPropertyKey::AccessorGet(accessor_name)
        } else {
            ObjectPropertyKey::AccessorSet(accessor_name)
        };
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
                let key = if number.is_finite() && number.fract() == 0.0 {
                    format!("{number:.0}")
                } else {
                    number.to_string()
                };
                Ok(ObjectPropertyKey::Static(key))
            }
            TokenKind::LBracket => {
                self.advance();
                let expr = self.parse_expression_inner()?;
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

    fn parse_object_method_value(&mut self) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after method name")?;
        let params = self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        self.function_depth += 1;
        let body = self.parse_block_body(
            "expected '{' before method body",
            "expected '}' after method body",
        );
        self.function_depth = self.function_depth.saturating_sub(1);
        let body = body?;

        Ok(Expr::Function {
            name: None,
            params,
            body,
        })
    }

    fn parse_array_literal(&mut self) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LBracket, "expected '[' before array literal")?;
        let mut elements = Vec::new();
        if self.matches(&TokenKind::RBracket) {
            return Ok(Expr::ArrayLiteral(elements));
        }
        loop {
            elements.push(self.parse_expression_inner()?);
            if self.matches(&TokenKind::Comma) {
                if self.check(&TokenKind::RBracket) {
                    break;
                }
                continue;
            }
            break;
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
    use super::{parse_expression, parse_script};
    use ast::{
        BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier,
        ObjectProperty, ObjectPropertyKey, Script, Stmt, SwitchCase, UnaryOp, VariableDeclaration,
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
        assert_eq!(parsed, Expr::Identifier(Identifier("eval".to_string())));
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
                    body: vec![Stmt::Return(Some(Expr::Number(1.0)))],
                },
            },
        ]);
        assert_eq!(parsed, expected);
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
                    body: vec![],
                },
            },
            ObjectProperty {
                key: ObjectPropertyKey::AccessorSet("foo".to_string()),
                value: Expr::Function {
                    name: None,
                    params: vec![Identifier("v".to_string())],
                    body: vec![],
                },
            },
        ]);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_function_expression() {
        let parsed = parse_expression("function add(a, b) { return a + b; }")
            .expect("parser should succeed");
        let expected = Expr::Function {
            name: Some(Identifier("add".to_string())),
            params: vec![Identifier("a".to_string()), Identifier("b".to_string())],
            body: vec![Stmt::Return(Some(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Identifier(Identifier("a".to_string()))),
                right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
            }))],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_arrow_function_with_empty_parameters() {
        let parsed = parse_expression("() => 1").expect("parser should succeed");
        let expected = Expr::Function {
            name: None,
            params: vec![],
            body: vec![Stmt::Return(Some(Expr::Number(1.0)))],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_arrow_function_with_single_parameter() {
        let parsed = parse_expression("x => x + 1").expect("parser should succeed");
        let expected = Expr::Function {
            name: None,
            params: vec![Identifier("x".to_string())],
            body: vec![Stmt::Return(Some(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                right: Box::new(Expr::Number(1.0)),
            }))],
        };
        assert_eq!(parsed, expected);
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
            arguments: vec![Expr::ArrayLiteral(vec![])],
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
        let expected = Expr::Assign {
            target: Identifier("x".to_string()),
            value: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                right: Box::new(Expr::Number(1.0)),
            }),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_postfix_increment_expression() {
        let parsed = parse_expression("x++").expect("parser should succeed");
        let expected = Expr::Assign {
            target: Identifier("x".to_string()),
            value: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                right: Box::new(Expr::Number(1.0)),
            }),
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
            Expr::String("ok".to_string())
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
        let expected = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("C".to_string()),
                    initializer: Some(Expr::ObjectLiteral(vec![])),
                }),
                Stmt::Expression(Expr::Identifier(Identifier("C".to_string()))),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_class_expression_baseline() {
        let parsed = parse_expression("class await {}").expect("parser should succeed");
        assert_eq!(parsed, Expr::ObjectLiteral(vec![]));
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
    fn parses_for_in_of_with_embedded_let_asi_baseline() {
        parse_script("for (var x in null) let\n{}").expect("script parsing should succeed");
        parse_script("for (var x of []) let\n{}").expect("script parsing should succeed");
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
    fn rejects_function_declaration_in_embedded_statement() {
        let err = parse_script("while (1) function f() {}").expect_err("parser should fail");
        assert_eq!(
            err.message,
            "function declaration not allowed in statement position"
        );
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
