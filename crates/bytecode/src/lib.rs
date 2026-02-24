#![forbid(unsafe_code)]

use ast::{
    BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier,
    ObjectPropertyKey, Script, Stmt, StringLiteral, SwitchCase, UnaryOp, UpdateTarget,
    VariableDeclaration,
};
use std::collections::BTreeSet;

const NON_SIMPLE_PARAMS_MARKER: &str = "$__qjs_non_simple_params__$";
const ARROW_FUNCTION_MARKER: &str = "$__qjs_arrow_function__$";
const PARAM_INIT_SCOPE_START_MARKER: &str = "$__qjs_param_init_scope_start__$";
const PARAM_INIT_SCOPE_END_MARKER: &str = "$__qjs_param_init_scope_end__$";
const REST_PARAM_MARKER_PREFIX: &str = "$__qjs_rest_param__$";
const CLASS_METHOD_NO_PROTOTYPE_MARKER: &str = "$__qjs_class_method_no_prototype__$";
const NAMED_FUNCTION_EXPR_MARKER: &str = "$__qjs_named_function_expr__$";
const CLASS_CONSTRUCTOR_SUPER_BASE_BINDING: &str = "$__qjs_super_base__$";

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    LoadNumber(f64),
    LoadBool(bool),
    LoadNull,
    LoadString(String),
    LoadUndefined,
    LoadUninitialized,
    CreateObject,
    CreateArray,
    LoadIdentifier(String),
    LoadFunction(usize),
    DefineVariable {
        name: String,
        mutable: bool,
    },
    DefineVar(String),
    DefineFunction {
        name: String,
        function_id: usize,
    },
    StoreVariable(String),
    GetProperty(String),
    GetPropertyByValue,
    GetSuperProperty(String),
    GetSuperPropertyByValue,
    PrepareSuperMethod(String),
    PrepareSuperMethodByValue,
    DefineProperty(String),
    DefineProtoProperty,
    DefineArrayLength,
    ArrayAppend,
    ArrayAppendSpread,
    ArrayElision,
    DefineGetter(String),
    DefineSetter(String),
    DefineGetterByValue,
    DefineSetterByValue,
    SetProperty(String),
    SetPropertyByValue,
    SetSuperProperty(String),
    SetSuperPropertyByValue,
    DeleteIdentifier(String),
    DeleteProperty(String),
    DeletePropertyByValue,
    DeleteSuperProperty,
    ResolveIdentifierReference(String),
    LoadReferenceValue,
    StoreReferenceValue,
    EnterScope,
    ExitScope,
    EnterParamInitScope,
    ExitParamInitScope,
    EnterWith,
    ExitWith,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    Neg,
    Not,
    BitNot,
    Typeof,
    TypeofIdentifier(String),
    ToNumber,
    ToPropertyKey,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    InstanceOf,
    JumpIfFalse(usize),
    Jump(usize),
    PushExceptionHandler {
        catch_target: Option<usize>,
        finally_target: Option<usize>,
    },
    PopExceptionHandler,
    LoadException,
    RethrowIfException,
    Throw,
    Call(usize),
    CallWithSpread(Vec<bool>),
    CallIdentifier {
        name: String,
        arg_count: usize,
    },
    CallIdentifierWithSpread {
        name: String,
        spread_flags: Vec<bool>,
    },
    CallMethod(usize),
    CallMethodWithSpread(Vec<bool>),
    Construct(usize),
    ConstructWithSpread(Vec<bool>),
    MarkStrict,
    Return,
    Dup,
    Dup2,
    Dup3,
    Swap,
    RotRight4,
    RotRight5,
    Pop,
    Nop,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompiledFunction {
    pub name: String,
    pub length: usize,
    pub params: Vec<String>,
    pub code: Vec<Opcode>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chunk {
    pub code: Vec<Opcode>,
    pub functions: Vec<CompiledFunction>,
}

#[derive(Debug, Default)]
struct Compiler {
    functions: Vec<CompiledFunction>,
    scope_depth: usize,
    handler_depth: usize,
    loops: Vec<LoopContext>,
    break_contexts: Vec<BreakContext>,
    label_contexts: Vec<LabelContext>,
    finally_contexts: Vec<FinallyContext>,
    next_switch_temp_id: usize,
    loop_completion_targets: Vec<String>,
    function_nesting: usize,
}

#[derive(Debug, Default)]
struct LoopContext {
    scope_depth: usize,
    handler_depth: usize,
    continue_jumps: Vec<usize>,
}

#[derive(Debug, Default)]
struct BreakContext {
    scope_depth: usize,
    handler_depth: usize,
    break_jumps: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LabelContext {
    name: String,
    break_context_index: usize,
    continue_loop_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
struct FinallyContext {
    handler_depth: usize,
    action: FinallyAction,
}

#[derive(Debug, Clone, PartialEq)]
enum FinallyAction {
    Statements(Vec<Stmt>),
    ExitWith,
}

pub fn compile_script(script: &Script) -> Chunk {
    let mut compiler = Compiler::default();
    let mut code = Vec::new();
    let produced_value = compiler.compile_statement_list(&script.statements, &mut code, true);
    if !produced_value {
        code.push(Opcode::LoadUndefined);
    }
    code.push(Opcode::Halt);
    Chunk {
        code,
        functions: compiler.functions,
    }
}

pub fn compile_expression(expr: &Expr) -> Chunk {
    let mut compiler = Compiler::default();
    let mut code = Vec::new();
    compiler.compile_expr(expr, &mut code);
    code.push(Opcode::Halt);
    Chunk {
        code,
        functions: compiler.functions,
    }
}

impl Compiler {
    fn compile_statement_list(
        &mut self,
        statements: &[Stmt],
        code: &mut Vec<Opcode>,
        preserve_value: bool,
    ) -> bool {
        if self.statement_list_has_use_strict_directive(statements) {
            code.push(Opcode::MarkStrict);
        }
        if self.scope_depth == 0 {
            let mut hoisted_var_names = BTreeSet::new();
            self.collect_hoisted_var_names(statements, &mut hoisted_var_names);
            for name in hoisted_var_names {
                code.push(Opcode::DefineVar(name));
            }
        }
        let mut hoisted_let_names = BTreeSet::new();
        self.collect_hoisted_let_names(statements, &mut hoisted_let_names);
        for name in hoisted_let_names {
            code.push(Opcode::LoadUninitialized);
            code.push(Opcode::DefineVariable {
                name,
                mutable: true,
            });
        }

        // Function declarations are hoisted to the top of their containing scope.
        for stmt in statements {
            if let Stmt::FunctionDeclaration(FunctionDeclaration { name, params, body }) = stmt {
                let function_id = self.compile_function(Some(name), params, body);
                code.push(Opcode::DefineFunction {
                    name: name.0.clone(),
                    function_id,
                });
            }
        }

        let executable_indexes: Vec<usize> = statements
            .iter()
            .enumerate()
            .filter_map(|(idx, stmt)| {
                if matches!(stmt, Stmt::FunctionDeclaration(_)) {
                    None
                } else {
                    Some(idx)
                }
            })
            .collect();
        let last_value_candidate = if preserve_value {
            statements.iter().enumerate().rev().find_map(|(idx, stmt)| {
                if Self::statement_has_static_empty_completion(stmt) {
                    None
                } else {
                    Some(idx)
                }
            })
        } else {
            None
        };

        let mut produced_value = false;
        let last_executable = if preserve_value {
            last_value_candidate.or(executable_indexes.last().copied())
        } else {
            executable_indexes.last().copied()
        };
        for (index, stmt) in statements.iter().enumerate() {
            if matches!(stmt, Stmt::FunctionDeclaration(_)) {
                code.push(Opcode::Nop);
                continue;
            }
            let keep_value = preserve_value && Some(index) == last_executable;
            let statement_produced = self.compile_stmt(stmt, code, keep_value);
            if keep_value || !preserve_value {
                produced_value = statement_produced;
            }
        }

        produced_value
    }

    fn statement_list_has_static_empty_completion(statements: &[Stmt]) -> bool {
        for statement in statements {
            if matches!(statement, Stmt::FunctionDeclaration(_)) {
                continue;
            }
            if !Self::statement_has_static_empty_completion(statement) {
                return false;
            }
        }
        true
    }

    fn statement_has_static_empty_completion(statement: &Stmt) -> bool {
        match statement {
            Stmt::FunctionDeclaration(_)
            | Stmt::VariableDeclaration(_)
            | Stmt::VariableDeclarations(_)
            | Stmt::Empty => true,
            Stmt::Block(statements) => Self::statement_list_has_static_empty_completion(statements),
            _ => false,
        }
    }

    fn statement_list_has_use_strict_directive(&self, statements: &[Stmt]) -> bool {
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

    fn collect_hoisted_var_names(&self, statements: &[Stmt], names: &mut BTreeSet<String>) {
        for stmt in statements {
            self.collect_hoisted_var_names_from_stmt(stmt, names);
        }
    }

    fn collect_hoisted_var_names_from_stmt(&self, stmt: &Stmt, names: &mut BTreeSet<String>) {
        match stmt {
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Var,
                name: Identifier(binding_name),
                ..
            }) => {
                names.insert(binding_name.clone());
            }
            Stmt::VariableDeclarations(declarations) => {
                for declaration in declarations {
                    if declaration.kind == BindingKind::Var {
                        names.insert(declaration.name.0.clone());
                    }
                }
            }
            Stmt::FunctionDeclaration(_) => {}
            Stmt::Block(statements) => self.collect_hoisted_var_names(statements, names),
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.collect_hoisted_var_names_from_stmt(consequent, names);
                if let Some(alternate) = alternate {
                    self.collect_hoisted_var_names_from_stmt(alternate, names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::With { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Labeled { body, .. } => {
                self.collect_hoisted_var_names_from_stmt(body, names);
            }
            Stmt::For {
                initializer, body, ..
            } => {
                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VariableDeclaration(declaration) => {
                            if declaration.kind == BindingKind::Var {
                                names.insert(declaration.name.0.clone());
                            }
                        }
                        ForInitializer::VariableDeclarations(declarations) => {
                            for declaration in declarations {
                                if declaration.kind == BindingKind::Var {
                                    names.insert(declaration.name.0.clone());
                                }
                            }
                        }
                        ForInitializer::Expression(_) => {}
                    }
                }
                self.collect_hoisted_var_names_from_stmt(body, names);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    self.collect_hoisted_var_names(&case.consequent, names);
                }
            }
            Stmt::Try {
                try_block,
                catch_block,
                finally_block,
                ..
            } => {
                self.collect_hoisted_var_names(try_block, names);
                if let Some(catch_block) = catch_block {
                    self.collect_hoisted_var_names(catch_block, names);
                }
                if let Some(finally_block) = finally_block {
                    self.collect_hoisted_var_names(finally_block, names);
                }
            }
            _ => {}
        }
    }

    fn collect_hoisted_let_names(&self, statements: &[Stmt], names: &mut BTreeSet<String>) {
        for stmt in statements {
            match stmt {
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier(binding_name),
                    ..
                }) => {
                    names.insert(binding_name.clone());
                }
                Stmt::VariableDeclarations(declarations) => {
                    for declaration in declarations {
                        if declaration.kind == BindingKind::Let {
                            names.insert(declaration.name.0.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn compile_stmt(&mut self, stmt: &Stmt, code: &mut Vec<Opcode>, keep_value: bool) -> bool {
        match stmt {
            Stmt::Empty => {
                if keep_value {
                    code.push(Opcode::LoadUndefined);
                    true
                } else {
                    code.push(Opcode::Nop);
                    false
                }
            }
            Stmt::VariableDeclaration(VariableDeclaration {
                kind,
                name: Identifier(binding_name),
                initializer,
            }) => {
                if self.scope_depth == 0
                    && self.function_nesting == 0
                    && matches!(kind, BindingKind::Let | BindingKind::Const)
                    && matches!(binding_name.as_str(), "undefined" | "NaN" | "Infinity")
                {
                    code.push(Opcode::LoadString(format!(
                        "restricted global lexical binding: {binding_name}"
                    )));
                    code.push(Opcode::Throw);
                    return true;
                }
                if *kind == BindingKind::Var {
                    if let Some(expr) = initializer {
                        code.push(Opcode::ResolveIdentifierReference(binding_name.clone()));
                        self.compile_expr(expr, code);
                        code.push(Opcode::StoreReferenceValue);
                        code.push(Opcode::Pop);
                    } else {
                        code.push(Opcode::Nop);
                    }
                    return false;
                }
                if let Some(expr) = initializer {
                    self.compile_expr(expr, code);
                } else {
                    code.push(Opcode::LoadUndefined);
                }
                code.push(Opcode::DefineVariable {
                    name: binding_name.clone(),
                    mutable: matches!(kind, BindingKind::Let | BindingKind::Var),
                });
                false
            }
            Stmt::VariableDeclarations(declarations) => {
                for declaration in declarations {
                    self.compile_stmt(&Stmt::VariableDeclaration(declaration.clone()), code, false);
                }
                false
            }
            Stmt::FunctionDeclaration(FunctionDeclaration { name, params, body }) => {
                let function_id = self.compile_function(Some(name), params, body);
                code.push(Opcode::DefineFunction {
                    name: name.0.clone(),
                    function_id,
                });
                false
            }
            Stmt::Return(value) => {
                if let Some(expr) = value {
                    self.compile_expr(expr, code);
                } else {
                    code.push(Opcode::LoadUndefined);
                }
                self.emit_return_with_finally(code);
                true
            }
            Stmt::Expression(expr) => {
                if let Expr::String(StringLiteral { value, has_escape }) = expr {
                    if !*has_escape && value == PARAM_INIT_SCOPE_START_MARKER {
                        code.push(Opcode::EnterParamInitScope);
                        return false;
                    }
                    if !*has_escape && value == PARAM_INIT_SCOPE_END_MARKER {
                        code.push(Opcode::ExitParamInitScope);
                        return false;
                    }
                }
                self.compile_expr(expr, code);
                if keep_value {
                    true
                } else if let Some(name) = self.loop_completion_targets.last() {
                    code.push(Opcode::StoreVariable(name.clone()));
                    code.push(Opcode::Pop);
                    false
                } else {
                    code.push(Opcode::Pop);
                    false
                }
            }
            Stmt::Block(statements) => {
                code.push(Opcode::EnterScope);
                self.scope_depth += 1;
                let block_value = self.compile_statement_list(statements, code, keep_value);
                if keep_value && !block_value {
                    code.push(Opcode::LoadUndefined);
                }
                code.push(Opcode::ExitScope);
                self.scope_depth = self.scope_depth.saturating_sub(1);
                keep_value
            }
            Stmt::If {
                condition,
                consequent,
                alternate,
            } => {
                if !keep_value {
                    if let Some(name) = self.loop_completion_targets.last() {
                        code.push(Opcode::LoadUndefined);
                        code.push(Opcode::StoreVariable(name.clone()));
                        code.push(Opcode::Pop);
                    }
                }
                self.compile_expr(condition, code);
                let jump_to_alternate_pos = code.len();
                code.push(Opcode::JumpIfFalse(usize::MAX));

                let consequent_value = self.compile_stmt(consequent, code, keep_value);
                if keep_value && !consequent_value {
                    code.push(Opcode::LoadUndefined);
                }

                let jump_to_end_pos = code.len();
                code.push(Opcode::Jump(usize::MAX));

                let alternate_start = code.len();
                if let Some(alternate_stmt) = alternate {
                    let alternate_value = self.compile_stmt(alternate_stmt, code, keep_value);
                    if keep_value && !alternate_value {
                        code.push(Opcode::LoadUndefined);
                    }
                } else if keep_value {
                    code.push(Opcode::LoadUndefined);
                }

                let end = code.len();
                code[jump_to_alternate_pos] = Opcode::JumpIfFalse(alternate_start);
                code[jump_to_end_pos] = Opcode::Jump(end);
                keep_value
            }
            Stmt::While { condition, body } => {
                let completion_name = if keep_value {
                    let name = self.next_loop_completion_temp_name();
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::DefineVariable {
                        name: name.clone(),
                        mutable: true,
                    });
                    Some(name)
                } else {
                    None
                };
                let loop_start = code.len();
                self.compile_expr(condition, code);
                let jump_to_end_pos = code.len();
                code.push(Opcode::JumpIfFalse(usize::MAX));
                if let Some(name) = &completion_name {
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::StoreVariable(name.clone()));
                    code.push(Opcode::Pop);
                }
                self.break_contexts.push(BreakContext {
                    scope_depth: self.scope_depth,
                    handler_depth: self.handler_depth,
                    break_jumps: Vec::new(),
                });
                self.loops.push(LoopContext {
                    scope_depth: self.scope_depth,
                    handler_depth: self.handler_depth,
                    continue_jumps: Vec::new(),
                });

                if let Some(name) = &completion_name {
                    self.loop_completion_targets.push(name.clone());
                }
                self.compile_stmt(body, code, false);
                if completion_name.is_some() {
                    self.loop_completion_targets
                        .pop()
                        .expect("loop completion target should exist");
                }
                let continue_target = loop_start;
                code.push(Opcode::Jump(continue_target));

                let loop_end = code.len();
                code[jump_to_end_pos] = Opcode::JumpIfFalse(loop_end);
                let break_context = self
                    .break_contexts
                    .pop()
                    .expect("loop break context should exist");
                let loop_context = self.loops.pop().expect("loop context should exist");
                self.patch_loop_exits(loop_context, continue_target, code);
                self.patch_break_exits(break_context, loop_end, code);

                if let Some(name) = completion_name {
                    code.push(Opcode::LoadIdentifier(name));
                    true
                } else {
                    false
                }
            }
            Stmt::With { object, body } => {
                self.compile_with_statement(object, body, code, keep_value)
            }
            Stmt::DoWhile { body, condition } => {
                let completion_name = if keep_value {
                    let name = self.next_loop_completion_temp_name();
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::DefineVariable {
                        name: name.clone(),
                        mutable: true,
                    });
                    Some(name)
                } else {
                    None
                };
                let loop_start = code.len();
                if let Some(name) = &completion_name {
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::StoreVariable(name.clone()));
                    code.push(Opcode::Pop);
                }
                self.break_contexts.push(BreakContext {
                    scope_depth: self.scope_depth,
                    handler_depth: self.handler_depth,
                    break_jumps: Vec::new(),
                });
                self.loops.push(LoopContext {
                    scope_depth: self.scope_depth,
                    handler_depth: self.handler_depth,
                    continue_jumps: Vec::new(),
                });

                if let Some(name) = &completion_name {
                    self.loop_completion_targets.push(name.clone());
                }
                self.compile_stmt(body, code, false);
                if completion_name.is_some() {
                    self.loop_completion_targets
                        .pop()
                        .expect("loop completion target should exist");
                }
                let continue_target = code.len();
                self.compile_expr(condition, code);
                let jump_to_end_pos = code.len();
                code.push(Opcode::JumpIfFalse(usize::MAX));
                code.push(Opcode::Jump(loop_start));

                let loop_end = code.len();
                code[jump_to_end_pos] = Opcode::JumpIfFalse(loop_end);
                let break_context = self
                    .break_contexts
                    .pop()
                    .expect("loop break context should exist");
                let loop_context = self.loops.pop().expect("loop context should exist");
                self.patch_loop_exits(loop_context, continue_target, code);
                self.patch_break_exits(break_context, loop_end, code);

                if let Some(name) = completion_name {
                    code.push(Opcode::LoadIdentifier(name));
                    true
                } else {
                    false
                }
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
            } => {
                code.push(Opcode::EnterScope);
                self.scope_depth += 1;
                let outer_loop_scope_depth = self.scope_depth;

                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VariableDeclaration(declaration) => {
                            self.compile_stmt(
                                &Stmt::VariableDeclaration(declaration.clone()),
                                code,
                                false,
                            );
                        }
                        ForInitializer::VariableDeclarations(declarations) => {
                            self.compile_stmt(
                                &Stmt::VariableDeclarations(declarations.clone()),
                                code,
                                false,
                            );
                        }
                        ForInitializer::Expression(expr) => {
                            self.compile_expr(expr, code);
                            code.push(Opcode::Pop);
                        }
                    }
                }

                let for_lexical_bindings =
                    Self::for_initializer_lexical_bindings(initializer.as_ref());
                let needs_per_iteration_scope = !for_lexical_bindings.is_empty();
                let mut carry_temps = Vec::new();
                if needs_per_iteration_scope {
                    for (name, mutable) in &for_lexical_bindings {
                        if !*mutable {
                            continue;
                        }
                        let temp_name = self.next_loop_completion_temp_name();
                        code.push(Opcode::LoadUndefined);
                        code.push(Opcode::DefineVariable {
                            name: temp_name.clone(),
                            mutable: true,
                        });
                        code.push(Opcode::ResolveIdentifierReference(temp_name.clone()));
                        self.compile_expr(&Expr::Identifier(Identifier(name.clone())), code);
                        code.push(Opcode::StoreReferenceValue);
                        code.push(Opcode::Pop);
                        carry_temps.push((name.clone(), temp_name));
                    }
                }
                let completion_name = if keep_value {
                    let name = self.next_loop_completion_temp_name();
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::DefineVariable {
                        name: name.clone(),
                        mutable: true,
                    });
                    Some(name)
                } else {
                    None
                };

                let loop_start = code.len();
                if needs_per_iteration_scope {
                    code.push(Opcode::EnterScope);
                    self.scope_depth += 1;
                    for (name, mutable) in &for_lexical_bindings {
                        if let Some(temp_name) =
                            carry_temps.iter().find_map(|(binding_name, temp_name)| {
                                if binding_name == name {
                                    Some(temp_name.clone())
                                } else {
                                    None
                                }
                            })
                        {
                            self.compile_expr(&Expr::Identifier(Identifier(temp_name)), code);
                        } else {
                            self.compile_expr(&Expr::Identifier(Identifier(name.clone())), code);
                        }
                        code.push(Opcode::DefineVariable {
                            name: name.clone(),
                            mutable: *mutable,
                        });
                    }
                }
                let jump_to_end_pos = if let Some(condition) = condition {
                    self.compile_expr(condition, code);
                    let jump_to_end_pos = code.len();
                    code.push(Opcode::JumpIfFalse(usize::MAX));
                    Some(jump_to_end_pos)
                } else {
                    None
                };
                if let Some(name) = &completion_name {
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::StoreVariable(name.clone()));
                    code.push(Opcode::Pop);
                }

                self.break_contexts.push(BreakContext {
                    scope_depth: if needs_per_iteration_scope {
                        outer_loop_scope_depth
                    } else {
                        self.scope_depth
                    },
                    handler_depth: self.handler_depth,
                    break_jumps: Vec::new(),
                });
                self.loops.push(LoopContext {
                    scope_depth: self.scope_depth,
                    handler_depth: self.handler_depth,
                    continue_jumps: Vec::new(),
                });

                if let Some(name) = &completion_name {
                    self.loop_completion_targets.push(name.clone());
                }
                self.compile_stmt(body, code, false);
                if completion_name.is_some() {
                    self.loop_completion_targets
                        .pop()
                        .expect("loop completion target should exist");
                }
                let continue_target = code.len();
                if needs_per_iteration_scope {
                    for (name, temp_name) in &carry_temps {
                        code.push(Opcode::ResolveIdentifierReference(temp_name.clone()));
                        self.compile_expr(&Expr::Identifier(Identifier(name.clone())), code);
                        code.push(Opcode::StoreReferenceValue);
                        code.push(Opcode::Pop);
                    }
                    code.push(Opcode::ExitScope);
                    self.scope_depth = self.scope_depth.saturating_sub(1);
                    if let Some(update) = update {
                        code.push(Opcode::EnterScope);
                        self.scope_depth += 1;
                        for (name, mutable) in &for_lexical_bindings {
                            if let Some(temp_name) =
                                carry_temps.iter().find_map(|(binding_name, temp_name)| {
                                    if binding_name == name {
                                        Some(temp_name.clone())
                                    } else {
                                        None
                                    }
                                })
                            {
                                self.compile_expr(&Expr::Identifier(Identifier(temp_name)), code);
                            } else {
                                self.compile_expr(
                                    &Expr::Identifier(Identifier(name.clone())),
                                    code,
                                );
                            }
                            code.push(Opcode::DefineVariable {
                                name: name.clone(),
                                mutable: *mutable,
                            });
                        }
                        self.compile_expr(update, code);
                        code.push(Opcode::Pop);
                        for (name, temp_name) in &carry_temps {
                            code.push(Opcode::ResolveIdentifierReference(temp_name.clone()));
                            self.compile_expr(&Expr::Identifier(Identifier(name.clone())), code);
                            code.push(Opcode::StoreReferenceValue);
                            code.push(Opcode::Pop);
                        }
                        code.push(Opcode::ExitScope);
                        self.scope_depth = self.scope_depth.saturating_sub(1);
                    }
                } else if let Some(update) = update {
                    self.compile_expr(update, code);
                    code.push(Opcode::Pop);
                }
                code.push(Opcode::Jump(loop_start));

                let mut condition_false_jump = None;
                let condition_false_target =
                    if needs_per_iteration_scope && jump_to_end_pos.is_some() {
                        let target = code.len();
                        code.push(Opcode::ExitScope);
                        let jump_pos = code.len();
                        code.push(Opcode::Jump(usize::MAX));
                        condition_false_jump = Some(jump_pos);
                        Some(target)
                    } else {
                        None
                    };

                let loop_end = code.len();
                if let Some(jump_to_end_pos) = jump_to_end_pos {
                    code[jump_to_end_pos] =
                        Opcode::JumpIfFalse(condition_false_target.unwrap_or(loop_end));
                }
                if let Some(jump_pos) = condition_false_jump {
                    code[jump_pos] = Opcode::Jump(loop_end);
                }
                let break_context = self
                    .break_contexts
                    .pop()
                    .expect("loop break context should exist");
                let loop_context = self.loops.pop().expect("loop context should exist");
                self.patch_loop_exits(loop_context, continue_target, code);
                self.patch_break_exits(break_context, loop_end, code);
                if let Some(name) = completion_name {
                    code.push(Opcode::LoadIdentifier(name));
                }

                code.push(Opcode::ExitScope);
                self.scope_depth = self.scope_depth.saturating_sub(1);

                keep_value
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                let temp_name = self.next_switch_temp_name();
                let completion_name = if keep_value {
                    let name = self.next_loop_completion_temp_name();
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::DefineVariable {
                        name: name.clone(),
                        mutable: true,
                    });
                    Some(name)
                } else {
                    None
                };
                code.push(Opcode::EnterScope);
                self.scope_depth += 1;

                self.compile_expr(discriminant, code);
                code.push(Opcode::DefineVariable {
                    name: temp_name.clone(),
                    mutable: false,
                });

                let mut case_start_positions = vec![usize::MAX; cases.len()];
                let mut case_dispatch_jumps = vec![usize::MAX; cases.len()];
                let mut default_case_index = None;

                for (index, case) in cases.iter().enumerate() {
                    if let Some(test) = &case.test {
                        self.compile_expr(&Expr::Identifier(Identifier(temp_name.clone())), code);
                        self.compile_expr(test, code);
                        code.push(Opcode::StrictEq);

                        let jump_if_false_pos = code.len();
                        code.push(Opcode::JumpIfFalse(usize::MAX));
                        let jump_to_case_pos = code.len();
                        code.push(Opcode::Jump(usize::MAX));
                        let next_test_pos = code.len();
                        code[jump_if_false_pos] = Opcode::JumpIfFalse(next_test_pos);
                        case_dispatch_jumps[index] = jump_to_case_pos;
                    } else {
                        default_case_index = Some(index);
                    }
                }

                let jump_no_match_pos = code.len();
                code.push(Opcode::Jump(usize::MAX));

                self.break_contexts.push(BreakContext {
                    scope_depth: self.scope_depth,
                    handler_depth: self.handler_depth,
                    break_jumps: Vec::new(),
                });
                if let Some(name) = &completion_name {
                    self.loop_completion_targets.push(name.clone());
                }
                for (index, SwitchCase { consequent, .. }) in cases.iter().enumerate() {
                    case_start_positions[index] = code.len();
                    self.compile_statement_list(consequent, code, false);
                }
                if completion_name.is_some() {
                    self.loop_completion_targets
                        .pop()
                        .expect("switch completion target should exist");
                }

                let switch_end = code.len();
                let break_context = self
                    .break_contexts
                    .pop()
                    .expect("switch break context should exist");
                self.patch_break_exits(break_context, switch_end, code);

                for (index, jump_pos) in case_dispatch_jumps.into_iter().enumerate() {
                    if jump_pos != usize::MAX {
                        code[jump_pos] = Opcode::Jump(case_start_positions[index]);
                    }
                }
                let no_match_target = default_case_index
                    .map(|idx| case_start_positions[idx])
                    .unwrap_or(switch_end);
                code[jump_no_match_pos] = Opcode::Jump(no_match_target);

                if let Some(name) = completion_name {
                    code.push(Opcode::LoadIdentifier(name));
                }
                code.push(Opcode::ExitScope);
                self.scope_depth = self.scope_depth.saturating_sub(1);

                keep_value
            }
            Stmt::Throw(expr) => {
                self.compile_expr(expr, code);
                code.push(Opcode::Throw);
                true
            }
            Stmt::Try {
                try_block,
                catch_param,
                catch_block,
                finally_block,
            } => self.compile_try_statement(
                try_block,
                catch_param,
                catch_block,
                finally_block,
                code,
                keep_value,
            ),
            Stmt::Labeled { label, body } => {
                let break_context_index = self.break_contexts.len();
                self.break_contexts.push(BreakContext {
                    scope_depth: self.scope_depth,
                    handler_depth: self.handler_depth,
                    break_jumps: Vec::new(),
                });
                self.label_contexts.push(LabelContext {
                    name: label.0.clone(),
                    break_context_index,
                    continue_loop_index: if Self::statement_allows_continue_label(body) {
                        Some(self.loops.len())
                    } else {
                        None
                    },
                });

                let completion_name = if keep_value {
                    let name = self.next_loop_completion_temp_name();
                    code.push(Opcode::LoadUndefined);
                    code.push(Opcode::DefineVariable {
                        name: name.clone(),
                        mutable: true,
                    });
                    self.loop_completion_targets.push(name.clone());
                    Some(name)
                } else {
                    None
                };
                let body_value =
                    self.compile_stmt(body, code, keep_value && completion_name.is_none());
                if completion_name.is_some() {
                    self.loop_completion_targets
                        .pop()
                        .expect("label completion target should exist");
                }
                let label_end = code.len();
                self.label_contexts.pop();
                let break_context = self
                    .break_contexts
                    .pop()
                    .expect("label break context should exist");
                self.patch_break_exits(break_context, label_end, code);
                if let Some(name) = completion_name {
                    code.push(Opcode::LoadIdentifier(name));
                    true
                } else {
                    body_value
                }
            }
            Stmt::Break => {
                let (target_handler_depth, target_scope_depth) = {
                    let break_context = self
                        .break_contexts
                        .last()
                        .expect("break outside breakable context");
                    (break_context.handler_depth, break_context.scope_depth)
                };
                let jump_pos =
                    self.emit_jump_with_finally(code, target_handler_depth, target_scope_depth);
                self.break_contexts
                    .last_mut()
                    .expect("break outside breakable context")
                    .break_jumps
                    .push(jump_pos);
                false
            }
            Stmt::BreakLabel(label) => {
                let break_context_index = self
                    .label_contexts
                    .iter()
                    .rev()
                    .find(|context| context.name == label.0)
                    .map(|context| context.break_context_index)
                    .expect("break label should be resolved by parser");
                let (target_handler_depth, target_scope_depth) = {
                    let break_context = self
                        .break_contexts
                        .get(break_context_index)
                        .expect("break context for label should exist");
                    (break_context.handler_depth, break_context.scope_depth)
                };
                let jump_pos =
                    self.emit_jump_with_finally(code, target_handler_depth, target_scope_depth);
                self.break_contexts[break_context_index]
                    .break_jumps
                    .push(jump_pos);
                false
            }
            Stmt::Continue => {
                let (target_handler_depth, target_scope_depth) = {
                    let loop_context = self.loops.last().expect("continue outside loop");
                    (loop_context.handler_depth, loop_context.scope_depth)
                };
                let jump_pos =
                    self.emit_jump_with_finally(code, target_handler_depth, target_scope_depth);
                self.loops
                    .last_mut()
                    .expect("continue outside loop")
                    .continue_jumps
                    .push(jump_pos);
                false
            }
            Stmt::ContinueLabel(label) => {
                let continue_loop_index = self
                    .label_contexts
                    .iter()
                    .rev()
                    .find(|context| context.name == label.0)
                    .and_then(|context| context.continue_loop_index)
                    .expect("continue label should resolve to iteration statement");
                let (target_handler_depth, target_scope_depth) = {
                    let loop_context = self
                        .loops
                        .get(continue_loop_index)
                        .expect("loop context for continue label should exist");
                    (loop_context.handler_depth, loop_context.scope_depth)
                };
                let jump_pos =
                    self.emit_jump_with_finally(code, target_handler_depth, target_scope_depth);
                self.loops[continue_loop_index]
                    .continue_jumps
                    .push(jump_pos);
                false
            }
        }
    }

    fn compile_try_statement(
        &mut self,
        try_block: &[Stmt],
        catch_param: &Option<Identifier>,
        catch_block: &Option<Vec<Stmt>>,
        finally_block: &Option<Vec<Stmt>>,
        code: &mut Vec<Opcode>,
        keep_value: bool,
    ) -> bool {
        match (catch_block, finally_block) {
            (Some(catch_block), Some(finally_block)) => {
                let nested_try = Stmt::Try {
                    try_block: try_block.to_vec(),
                    catch_param: catch_param.clone(),
                    catch_block: Some(catch_block.clone()),
                    finally_block: None,
                };
                self.compile_try_finally(&[nested_try], finally_block, code, keep_value)
            }
            (Some(catch_block), None) => {
                self.compile_try_catch(try_block, catch_param, catch_block, code, keep_value)
            }
            (None, Some(finally_block)) => {
                self.compile_try_finally(try_block, finally_block, code, keep_value)
            }
            (None, None) => {
                if keep_value {
                    code.push(Opcode::LoadUndefined);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn compile_try_catch(
        &mut self,
        try_block: &[Stmt],
        catch_param: &Option<Identifier>,
        catch_block: &[Stmt],
        code: &mut Vec<Opcode>,
        keep_value: bool,
    ) -> bool {
        let completion_name = if keep_value {
            let name = self.next_loop_completion_temp_name();
            code.push(Opcode::LoadUndefined);
            code.push(Opcode::DefineVariable {
                name: name.clone(),
                mutable: true,
            });
            Some(name)
        } else {
            None
        };
        let handler_pos = code.len();
        code.push(Opcode::PushExceptionHandler {
            catch_target: None,
            finally_target: None,
        });
        self.handler_depth += 1;

        code.push(Opcode::EnterScope);
        self.scope_depth += 1;
        let try_value = self.compile_statement_list(try_block, code, keep_value);
        if let Some(name) = &completion_name {
            if try_value {
                code.push(Opcode::StoreVariable(name.clone()));
                code.push(Opcode::Pop);
            } else {
                code.push(Opcode::LoadUndefined);
                code.push(Opcode::StoreVariable(name.clone()));
                code.push(Opcode::Pop);
            }
        }
        code.push(Opcode::ExitScope);
        self.scope_depth = self.scope_depth.saturating_sub(1);
        code.push(Opcode::PopExceptionHandler);
        self.handler_depth = self.handler_depth.saturating_sub(1);

        let jump_after_catch_pos = code.len();
        code.push(Opcode::Jump(usize::MAX));
        let catch_start = code.len();
        code[handler_pos] = Opcode::PushExceptionHandler {
            catch_target: Some(catch_start),
            finally_target: None,
        };

        code.push(Opcode::EnterScope);
        self.scope_depth += 1;
        match catch_param {
            Some(Identifier(name)) => {
                code.push(Opcode::LoadException);
                code.push(Opcode::DefineVariable {
                    name: name.clone(),
                    mutable: true,
                });
            }
            None => {
                code.push(Opcode::LoadException);
                code.push(Opcode::Pop);
            }
        }

        let catch_value = self.compile_statement_list(catch_block, code, keep_value);
        if let Some(name) = &completion_name {
            if catch_value {
                code.push(Opcode::StoreVariable(name.clone()));
                code.push(Opcode::Pop);
            } else {
                code.push(Opcode::LoadUndefined);
                code.push(Opcode::StoreVariable(name.clone()));
                code.push(Opcode::Pop);
            }
        }
        code.push(Opcode::ExitScope);
        self.scope_depth = self.scope_depth.saturating_sub(1);
        let end = code.len();
        code[jump_after_catch_pos] = Opcode::Jump(end);

        if let Some(name) = completion_name {
            code.push(Opcode::LoadIdentifier(name));
            true
        } else {
            false
        }
    }

    fn compile_try_finally(
        &mut self,
        try_block: &[Stmt],
        finally_block: &[Stmt],
        code: &mut Vec<Opcode>,
        keep_value: bool,
    ) -> bool {
        let completion_name = if keep_value {
            let name = self.next_loop_completion_temp_name();
            code.push(Opcode::LoadUndefined);
            code.push(Opcode::DefineVariable {
                name: name.clone(),
                mutable: true,
            });
            Some(name)
        } else {
            None
        };
        let handler_pos = code.len();
        code.push(Opcode::PushExceptionHandler {
            catch_target: None,
            finally_target: None,
        });
        self.handler_depth += 1;
        self.finally_contexts.push(FinallyContext {
            handler_depth: self.handler_depth,
            action: FinallyAction::Statements(finally_block.to_vec()),
        });

        code.push(Opcode::EnterScope);
        self.scope_depth += 1;
        let try_value = self.compile_statement_list(try_block, code, keep_value);
        if let Some(name) = &completion_name {
            if try_value {
                code.push(Opcode::StoreVariable(name.clone()));
                code.push(Opcode::Pop);
            } else {
                code.push(Opcode::LoadUndefined);
                code.push(Opcode::StoreVariable(name.clone()));
                code.push(Opcode::Pop);
            }
        }
        code.push(Opcode::ExitScope);
        self.scope_depth = self.scope_depth.saturating_sub(1);
        self.finally_contexts.pop();
        code.push(Opcode::PopExceptionHandler);
        self.handler_depth = self.handler_depth.saturating_sub(1);

        let jump_to_finally_pos = code.len();
        code.push(Opcode::Jump(usize::MAX));
        let finally_start = code.len();
        code[handler_pos] = Opcode::PushExceptionHandler {
            catch_target: None,
            finally_target: Some(finally_start),
        };
        code[jump_to_finally_pos] = Opcode::Jump(finally_start);

        self.compile_scoped_statement_list(finally_block, code);
        code.push(Opcode::RethrowIfException);

        if let Some(name) = completion_name {
            code.push(Opcode::LoadIdentifier(name));
            true
        } else {
            false
        }
    }

    fn compile_with_statement(
        &mut self,
        object: &Expr,
        body: &Stmt,
        code: &mut Vec<Opcode>,
        keep_value: bool,
    ) -> bool {
        if !keep_value {
            if let Some(name) = self.loop_completion_targets.last() {
                code.push(Opcode::LoadUndefined);
                code.push(Opcode::StoreVariable(name.clone()));
                code.push(Opcode::Pop);
            }
        }
        self.compile_expr(object, code);
        code.push(Opcode::EnterWith);

        let handler_pos = code.len();
        code.push(Opcode::PushExceptionHandler {
            catch_target: None,
            finally_target: None,
        });
        self.handler_depth += 1;
        self.finally_contexts.push(FinallyContext {
            handler_depth: self.handler_depth,
            action: FinallyAction::ExitWith,
        });

        let body_value = self.compile_stmt(body, code, keep_value);

        self.finally_contexts.pop();
        code.push(Opcode::PopExceptionHandler);
        self.handler_depth = self.handler_depth.saturating_sub(1);

        let jump_to_cleanup_pos = code.len();
        code.push(Opcode::Jump(usize::MAX));
        let cleanup_start = code.len();
        code[handler_pos] = Opcode::PushExceptionHandler {
            catch_target: None,
            finally_target: Some(cleanup_start),
        };
        code[jump_to_cleanup_pos] = Opcode::Jump(cleanup_start);

        code.push(Opcode::ExitWith);
        code.push(Opcode::RethrowIfException);

        if keep_value && !body_value {
            code.push(Opcode::LoadUndefined);
            true
        } else {
            body_value
        }
    }

    fn compile_function(
        &mut self,
        name: Option<&Identifier>,
        params: &[Identifier],
        body: &[Stmt],
    ) -> usize {
        let saved_scope_depth = self.scope_depth;
        let saved_handler_depth = self.handler_depth;
        let saved_loops = std::mem::take(&mut self.loops);
        let saved_break_contexts = std::mem::take(&mut self.break_contexts);
        let saved_label_contexts = std::mem::take(&mut self.label_contexts);
        let saved_finally_contexts = std::mem::take(&mut self.finally_contexts);
        let saved_loop_completion_targets = std::mem::take(&mut self.loop_completion_targets);
        let saved_function_nesting = self.function_nesting;
        self.scope_depth = 0;
        self.handler_depth = 0;
        self.function_nesting = self.function_nesting.saturating_add(1);

        let mut code = Vec::new();
        self.compile_statement_list(body, &mut code, false);
        code.push(Opcode::LoadUndefined);
        code.push(Opcode::Return);

        let function_id = self.functions.len();
        let length = Self::compute_expected_function_length(params, body);
        self.functions.push(CompiledFunction {
            name: name
                .map(|identifier| identifier.0.clone())
                .unwrap_or_else(|| "<anonymous>".to_string()),
            length,
            params: params.iter().map(|param| param.0.clone()).collect(),
            code,
        });

        self.scope_depth = saved_scope_depth;
        self.handler_depth = saved_handler_depth;
        self.loops = saved_loops;
        self.break_contexts = saved_break_contexts;
        self.label_contexts = saved_label_contexts;
        self.finally_contexts = saved_finally_contexts;
        self.loop_completion_targets = saved_loop_completion_targets;
        self.function_nesting = saved_function_nesting;
        function_id
    }

    fn compute_expected_function_length(params: &[Identifier], body: &[Stmt]) -> usize {
        if params.is_empty() {
            return 0;
        }
        let rest_index = Self::extract_rest_parameter_index(body);
        let mut defaulted = BTreeSet::new();
        for stmt in body {
            if Self::is_function_parameter_prelude(stmt) {
                continue;
            }
            let Some(name) = Self::extract_default_initializer_param(stmt) else {
                break;
            };
            defaulted.insert(name);
        }

        let default_index = params
            .iter()
            .position(|param| defaulted.contains(&param.0))
            .unwrap_or(params.len());
        rest_index.map_or(default_index, |index| default_index.min(index))
    }

    fn is_function_parameter_prelude(stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Expression(Expr::String(StringLiteral { value, .. })) => {
                value == NON_SIMPLE_PARAMS_MARKER
                    || value == ARROW_FUNCTION_MARKER
                    || value == PARAM_INIT_SCOPE_START_MARKER
                    || value == PARAM_INIT_SCOPE_END_MARKER
                    || value.starts_with(REST_PARAM_MARKER_PREFIX)
                    || value == CLASS_METHOD_NO_PROTOTYPE_MARKER
                    || value == NAMED_FUNCTION_EXPR_MARKER
                    || value == "use strict"
            }
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let,
                name: Identifier(name),
                ..
            }) => name == "super" || name == CLASS_CONSTRUCTOR_SUPER_BASE_BINDING,
            _ => false,
        }
    }

    fn extract_rest_parameter_index(body: &[Stmt]) -> Option<usize> {
        for stmt in body {
            let Stmt::Expression(Expr::String(StringLiteral { value, .. })) = stmt else {
                continue;
            };
            let Some(raw_index) = value.strip_prefix(REST_PARAM_MARKER_PREFIX) else {
                continue;
            };
            if let Ok(index) = raw_index.parse::<usize>() {
                return Some(index);
            }
        }
        None
    }

    fn extract_default_initializer_param(stmt: &Stmt) -> Option<String> {
        let Stmt::If {
            condition:
                Expr::Binary {
                    op: BinaryOp::StrictEqual,
                    left,
                    right,
                },
            consequent,
            alternate: None,
        } = stmt
        else {
            return None;
        };
        let Expr::Identifier(Identifier(condition_name)) = left.as_ref() else {
            return None;
        };
        let Expr::Identifier(Identifier(undefined_name)) = right.as_ref() else {
            return None;
        };
        if undefined_name != "undefined" {
            return None;
        }
        let Stmt::Expression(Expr::Assign {
            target: Identifier(target_name),
            ..
        }) = consequent.as_ref()
        else {
            return None;
        };
        if target_name != condition_name {
            return None;
        }
        Some(condition_name.clone())
    }

    fn statement_allows_continue_label(statement: &Stmt) -> bool {
        match statement {
            Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::For { .. } => true,
            Stmt::Labeled { body, .. } => Self::statement_allows_continue_label(body),
            _ => false,
        }
    }

    fn for_initializer_lexical_bindings(
        initializer: Option<&ForInitializer>,
    ) -> Vec<(String, bool)> {
        let Some(initializer) = initializer else {
            return Vec::new();
        };
        match initializer {
            ForInitializer::VariableDeclaration(declaration) => match declaration.kind {
                BindingKind::Let => vec![(declaration.name.0.clone(), true)],
                BindingKind::Const => vec![(declaration.name.0.clone(), false)],
                BindingKind::Var => Vec::new(),
            },
            ForInitializer::VariableDeclarations(declarations) => declarations
                .iter()
                .filter_map(|declaration| match declaration.kind {
                    BindingKind::Let => Some((declaration.name.0.clone(), true)),
                    BindingKind::Const => Some((declaration.name.0.clone(), false)),
                    BindingKind::Var => None,
                })
                .collect(),
            ForInitializer::Expression(_) => Vec::new(),
        }
    }

    fn patch_loop_exits(
        &self,
        loop_context: LoopContext,
        continue_target: usize,
        code: &mut [Opcode],
    ) {
        for jump_pos in loop_context.continue_jumps {
            code[jump_pos] = Opcode::Jump(continue_target);
        }
    }

    fn patch_break_exits(
        &self,
        break_context: BreakContext,
        break_target: usize,
        code: &mut [Opcode],
    ) {
        for jump_pos in break_context.break_jumps {
            code[jump_pos] = Opcode::Jump(break_target);
        }
    }

    fn emit_return_with_finally(&mut self, code: &mut Vec<Opcode>) {
        let saved_handler_depth = self.handler_depth;
        let saved_finally_contexts = self.finally_contexts.clone();

        let target_handler_depth = 0;
        self.emit_unwound_finally_blocks(code, target_handler_depth);
        Self::emit_handler_pops(self.handler_depth, target_handler_depth, code);
        self.handler_depth = target_handler_depth;
        code.push(Opcode::Return);

        self.handler_depth = saved_handler_depth;
        self.finally_contexts = saved_finally_contexts;
    }

    fn emit_jump_with_finally(
        &mut self,
        code: &mut Vec<Opcode>,
        target_handler_depth: usize,
        target_scope_depth: usize,
    ) -> usize {
        let saved_handler_depth = self.handler_depth;
        let saved_finally_contexts = self.finally_contexts.clone();

        self.emit_unwound_finally_blocks(code, target_handler_depth);
        Self::emit_handler_pops(self.handler_depth, target_handler_depth, code);
        self.handler_depth = target_handler_depth;
        Self::emit_scope_exits(self.scope_depth, target_scope_depth, code);
        let jump_pos = code.len();
        code.push(Opcode::Jump(usize::MAX));

        self.handler_depth = saved_handler_depth;
        self.finally_contexts = saved_finally_contexts;
        jump_pos
    }

    fn emit_unwound_finally_blocks(&mut self, code: &mut Vec<Opcode>, target_handler_depth: usize) {
        let unwind_count = self
            .finally_contexts
            .iter()
            .rev()
            .take_while(|ctx| ctx.handler_depth > target_handler_depth)
            .count();
        if unwind_count == 0 {
            return;
        }

        let remaining = self.finally_contexts.len().saturating_sub(unwind_count);
        let contexts_to_run = self.finally_contexts[remaining..].to_vec();
        self.finally_contexts.truncate(remaining);

        for context in contexts_to_run.iter().rev() {
            while self.handler_depth >= context.handler_depth {
                code.push(Opcode::PopExceptionHandler);
                self.handler_depth = self.handler_depth.saturating_sub(1);
            }
            match &context.action {
                FinallyAction::Statements(statements) => {
                    self.compile_scoped_statement_list(statements, code);
                }
                FinallyAction::ExitWith => {
                    code.push(Opcode::ExitWith);
                }
            }
        }
    }

    fn compile_scoped_statement_list(&mut self, statements: &[Stmt], code: &mut Vec<Opcode>) {
        code.push(Opcode::EnterScope);
        self.scope_depth += 1;
        let loop_completion_restore = self.loop_completion_targets.last().cloned().map(|name| {
            let saved_name = self.next_loop_completion_temp_name();
            code.push(Opcode::LoadIdentifier(name.clone()));
            code.push(Opcode::DefineVariable {
                name: saved_name.clone(),
                mutable: true,
            });
            code.push(Opcode::LoadUndefined);
            code.push(Opcode::StoreVariable(name.clone()));
            code.push(Opcode::Pop);
            (name, saved_name)
        });
        self.compile_statement_list(statements, code, false);
        if let Some((name, saved_name)) = loop_completion_restore {
            code.push(Opcode::LoadIdentifier(saved_name));
            code.push(Opcode::StoreVariable(name));
            code.push(Opcode::Pop);
        }
        code.push(Opcode::ExitScope);
        self.scope_depth = self.scope_depth.saturating_sub(1);
    }

    fn next_switch_temp_name(&mut self) -> String {
        let id = self.next_switch_temp_id;
        self.next_switch_temp_id += 1;
        format!("$__switch_tmp_{id}")
    }

    fn next_loop_completion_temp_name(&mut self) -> String {
        let id = self.next_switch_temp_id;
        self.next_switch_temp_id += 1;
        format!("$__loop_completion_{id}")
    }

    fn emit_handler_pops(current_depth: usize, target_depth: usize, code: &mut Vec<Opcode>) {
        let pops = current_depth.saturating_sub(target_depth);
        for _ in 0..pops {
            code.push(Opcode::PopExceptionHandler);
        }
    }

    fn emit_scope_exits(current_depth: usize, target_depth: usize, code: &mut Vec<Opcode>) {
        let exits = current_depth.saturating_sub(target_depth);
        for _ in 0..exits {
            code.push(Opcode::ExitScope);
        }
    }

    fn compile_expr(&mut self, expr: &Expr, code: &mut Vec<Opcode>) {
        match expr {
            Expr::Number(value) => code.push(Opcode::LoadNumber(*value)),
            Expr::Bool(value) => code.push(Opcode::LoadBool(*value)),
            Expr::Null => code.push(Opcode::LoadNull),
            Expr::String(StringLiteral { value, .. }) => {
                code.push(Opcode::LoadString(value.clone()))
            }
            Expr::RegexLiteral { pattern, flags } => {
                code.push(Opcode::LoadString(pattern.clone()));
                code.push(Opcode::LoadString(flags.clone()));
                code.push(Opcode::CallIdentifier {
                    name: "RegExp".to_string(),
                    arg_count: 2,
                });
            }
            Expr::This => code.push(Opcode::LoadIdentifier("this".to_string())),
            Expr::Function { name, params, body } => {
                let function_id = self.compile_function(name.as_ref(), params, body);
                code.push(Opcode::LoadFunction(function_id));
            }
            Expr::ObjectLiteral(properties) => {
                code.push(Opcode::CreateObject);
                for property in properties {
                    match &property.key {
                        ObjectPropertyKey::ProtoSetter => {
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineProtoProperty);
                        }
                        ObjectPropertyKey::Static(name) => {
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineProperty(name.clone()));
                        }
                        ObjectPropertyKey::AccessorGet(name) => {
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineGetter(name.clone()));
                        }
                        ObjectPropertyKey::AccessorSet(name) => {
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineSetter(name.clone()));
                        }
                        ObjectPropertyKey::Computed(key_expr) => {
                            code.push(Opcode::Dup);
                            self.compile_expr(key_expr, code);
                            code.push(Opcode::ToPropertyKey);
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::SetPropertyByValue);
                            code.push(Opcode::Pop);
                        }
                        ObjectPropertyKey::AccessorGetComputed(key_expr) => {
                            code.push(Opcode::Dup);
                            self.compile_expr(key_expr, code);
                            code.push(Opcode::ToPropertyKey);
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineGetterByValue);
                            code.push(Opcode::Pop);
                        }
                        ObjectPropertyKey::AccessorSetComputed(key_expr) => {
                            code.push(Opcode::Dup);
                            self.compile_expr(key_expr, code);
                            code.push(Opcode::ToPropertyKey);
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineSetterByValue);
                            code.push(Opcode::Pop);
                        }
                    }
                }
            }
            Expr::ArrayLiteral(elements) => {
                code.push(Opcode::CreateArray);
                for element in elements {
                    match element {
                        Expr::Elision => code.push(Opcode::ArrayElision),
                        Expr::SpreadArgument(inner) => {
                            self.compile_expr(inner, code);
                            code.push(Opcode::ArrayAppendSpread);
                        }
                        _ => {
                            self.compile_expr(element, code);
                            code.push(Opcode::ArrayAppend);
                        }
                    }
                }
            }
            Expr::Elision => code.push(Opcode::LoadUndefined),
            Expr::Unary { op, expr } => {
                if *op == UnaryOp::Void {
                    self.compile_expr(expr, code);
                    code.push(Opcode::Pop);
                    code.push(Opcode::LoadUndefined);
                    return;
                }
                if *op == UnaryOp::Delete {
                    match &**expr {
                        Expr::Identifier(Identifier(name)) => {
                            code.push(Opcode::DeleteIdentifier(name.clone()));
                        }
                        Expr::Member { object, property } => {
                            if Self::is_super_identifier(object) {
                                code.push(Opcode::DeleteSuperProperty);
                            } else {
                                self.compile_expr(object, code);
                                code.push(Opcode::DeleteProperty(property.clone()));
                            }
                        }
                        Expr::MemberComputed { object, property } => {
                            if Self::is_super_identifier(object) {
                                code.push(Opcode::DeleteSuperProperty);
                            } else {
                                self.compile_expr(object, code);
                                self.compile_expr(property, code);
                                code.push(Opcode::DeletePropertyByValue);
                            }
                        }
                        _ => {
                            self.compile_expr(expr, code);
                            code.push(Opcode::Pop);
                            code.push(Opcode::LoadBool(true));
                        }
                    }
                    return;
                }
                if *op == UnaryOp::Typeof {
                    if let Expr::Identifier(Identifier(name)) = &**expr {
                        code.push(Opcode::TypeofIdentifier(name.clone()));
                    } else {
                        self.compile_expr(expr, code);
                        code.push(Opcode::Typeof);
                    }
                    return;
                }
                self.compile_expr(expr, code);
                let opcode = match op {
                    UnaryOp::Plus => {
                        code.push(Opcode::LoadNumber(0.0));
                        code.push(Opcode::Sub);
                        return;
                    }
                    UnaryOp::Minus => Opcode::Neg,
                    UnaryOp::Not => Opcode::Not,
                    UnaryOp::BitNot => Opcode::BitNot,
                    UnaryOp::Typeof | UnaryOp::Void | UnaryOp::Delete => unreachable!(),
                };
                code.push(opcode);
            }
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => {
                self.compile_expr(condition, code);
                let jump_to_alternate_pos = code.len();
                code.push(Opcode::JumpIfFalse(usize::MAX));
                self.compile_expr(consequent, code);
                let jump_to_end_pos = code.len();
                code.push(Opcode::Jump(usize::MAX));
                let alternate_start = code.len();
                self.compile_expr(alternate, code);
                let end = code.len();
                code[jump_to_alternate_pos] = Opcode::JumpIfFalse(alternate_start);
                code[jump_to_end_pos] = Opcode::Jump(end);
            }
            Expr::Sequence(expressions) => {
                let mut iter = expressions.iter().peekable();
                if iter.peek().is_none() {
                    code.push(Opcode::LoadUndefined);
                    return;
                }
                while let Some(item) = iter.next() {
                    self.compile_expr(item, code);
                    if iter.peek().is_some() {
                        code.push(Opcode::Pop);
                    }
                }
            }
            Expr::Assign {
                target: Identifier(name),
                value,
            } => {
                code.push(Opcode::ResolveIdentifierReference(name.clone()));
                if let Some((op, right)) = Self::match_identifier_compound_value(name, value) {
                    code.push(Opcode::LoadReferenceValue);
                    self.compile_expr(right, code);
                    code.push(Self::binary_opcode(op));
                } else {
                    self.compile_expr(value, code);
                }
                code.push(Opcode::StoreReferenceValue);
            }
            Expr::AssignMember {
                object,
                property,
                value,
            } => {
                let is_super_member = Self::is_super_identifier(object);
                if let Some((op, right)) =
                    Self::match_member_compound_value(object, property, value)
                {
                    self.compile_expr(object, code);
                    if is_super_member {
                        code.push(Opcode::LoadIdentifier("this".to_string()));
                        code.push(Opcode::Dup2);
                        code.push(Opcode::GetSuperProperty(property.clone()));
                        self.compile_expr(right, code);
                        code.push(Self::binary_opcode(op));
                        code.push(Opcode::SetSuperProperty(property.clone()));
                    } else {
                        code.push(Opcode::Dup);
                        code.push(Opcode::GetProperty(property.clone()));
                        self.compile_expr(right, code);
                        code.push(Self::binary_opcode(op));
                        code.push(Opcode::SetProperty(property.clone()));
                    }
                } else {
                    self.compile_expr(object, code);
                    if is_super_member {
                        code.push(Opcode::LoadIdentifier("this".to_string()));
                        self.compile_expr(value, code);
                        code.push(Opcode::SetSuperProperty(property.clone()));
                    } else {
                        self.compile_expr(value, code);
                        code.push(Opcode::SetProperty(property.clone()));
                    }
                }
            }
            Expr::AssignMemberComputed {
                object,
                property,
                value,
            } => {
                let is_super_member = Self::is_super_identifier(object);
                if let Some((op, right)) =
                    Self::match_member_computed_compound_value(object, property, value)
                {
                    self.compile_expr(object, code);
                    if is_super_member {
                        code.push(Opcode::LoadIdentifier("this".to_string()));
                        self.compile_expr(property, code);
                        code.push(Opcode::Dup3);
                        code.push(Opcode::GetSuperPropertyByValue);
                        self.compile_expr(right, code);
                        code.push(Self::binary_opcode(op));
                        code.push(Opcode::SetSuperPropertyByValue);
                    } else {
                        self.compile_expr(property, code);
                        code.push(Opcode::Dup2);
                        code.push(Opcode::GetPropertyByValue);
                        self.compile_expr(right, code);
                        code.push(Self::binary_opcode(op));
                        code.push(Opcode::SetPropertyByValue);
                    }
                } else {
                    self.compile_expr(object, code);
                    if is_super_member {
                        code.push(Opcode::LoadIdentifier("this".to_string()));
                        self.compile_expr(property, code);
                        self.compile_expr(value, code);
                        code.push(Opcode::SetSuperPropertyByValue);
                    } else {
                        self.compile_expr(property, code);
                        self.compile_expr(value, code);
                        code.push(Opcode::SetPropertyByValue);
                    }
                }
            }
            Expr::Update {
                target,
                increment,
                prefix,
            } => self.compile_update_expression(target, *increment, *prefix, code),
            Expr::Identifier(Identifier(name)) => code.push(Opcode::LoadIdentifier(name.clone())),
            Expr::Member { object, property } => {
                self.compile_expr(object, code);
                if Self::is_super_identifier(object) {
                    code.push(Opcode::LoadIdentifier("this".to_string()));
                    code.push(Opcode::GetSuperProperty(property.clone()));
                } else {
                    code.push(Opcode::GetProperty(property.clone()));
                }
            }
            Expr::MemberComputed { object, property } => {
                self.compile_expr(object, code);
                if Self::is_super_identifier(object) {
                    code.push(Opcode::LoadIdentifier("this".to_string()));
                    self.compile_expr(property, code);
                    code.push(Opcode::GetSuperPropertyByValue);
                } else {
                    self.compile_expr(property, code);
                    code.push(Opcode::GetPropertyByValue);
                }
            }
            Expr::Call { callee, arguments } => match callee.as_ref() {
                Expr::Identifier(Identifier(name)) => {
                    let mut spread_flags = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        if let Expr::SpreadArgument(inner) = argument {
                            self.compile_expr(inner, code);
                            spread_flags.push(true);
                        } else {
                            self.compile_expr(argument, code);
                            spread_flags.push(false);
                        }
                    }
                    if spread_flags.iter().any(|is_spread| *is_spread) {
                        code.push(Opcode::CallIdentifierWithSpread {
                            name: name.clone(),
                            spread_flags,
                        });
                    } else {
                        code.push(Opcode::CallIdentifier {
                            name: name.clone(),
                            arg_count: arguments.len(),
                        });
                    }
                }
                Expr::Member { object, property } => {
                    let is_super_member = Self::is_super_identifier(object);
                    self.compile_expr(object, code);
                    if is_super_member {
                        code.push(Opcode::LoadIdentifier("this".to_string()));
                        code.push(Opcode::PrepareSuperMethod(property.clone()));
                    } else {
                        code.push(Opcode::Dup);
                        code.push(Opcode::GetProperty(property.clone()));
                    }
                    let mut spread_flags = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        if let Expr::SpreadArgument(inner) = argument {
                            self.compile_expr(inner, code);
                            spread_flags.push(true);
                        } else {
                            self.compile_expr(argument, code);
                            spread_flags.push(false);
                        }
                    }
                    if spread_flags.iter().any(|is_spread| *is_spread) {
                        code.push(Opcode::CallMethodWithSpread(spread_flags));
                    } else {
                        code.push(Opcode::CallMethod(arguments.len()));
                    }
                }
                Expr::MemberComputed { object, property } => {
                    let is_super_member = Self::is_super_identifier(object);
                    self.compile_expr(object, code);
                    if is_super_member {
                        code.push(Opcode::LoadIdentifier("this".to_string()));
                        self.compile_expr(property, code);
                        code.push(Opcode::PrepareSuperMethodByValue);
                    } else {
                        code.push(Opcode::Dup);
                        self.compile_expr(property, code);
                        code.push(Opcode::GetPropertyByValue);
                    }
                    let mut spread_flags = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        if let Expr::SpreadArgument(inner) = argument {
                            self.compile_expr(inner, code);
                            spread_flags.push(true);
                        } else {
                            self.compile_expr(argument, code);
                            spread_flags.push(false);
                        }
                    }
                    if spread_flags.iter().any(|is_spread| *is_spread) {
                        code.push(Opcode::CallMethodWithSpread(spread_flags));
                    } else {
                        code.push(Opcode::CallMethod(arguments.len()));
                    }
                }
                _ => {
                    self.compile_expr(callee, code);
                    let mut spread_flags = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        if let Expr::SpreadArgument(inner) = argument {
                            self.compile_expr(inner, code);
                            spread_flags.push(true);
                        } else {
                            self.compile_expr(argument, code);
                            spread_flags.push(false);
                        }
                    }
                    if spread_flags.iter().any(|is_spread| *is_spread) {
                        code.push(Opcode::CallWithSpread(spread_flags));
                    } else {
                        code.push(Opcode::Call(arguments.len()));
                    }
                }
            },
            Expr::New { callee, arguments } => {
                self.compile_expr(callee, code);
                let mut spread_flags = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    if let Expr::SpreadArgument(inner) = argument {
                        self.compile_expr(inner, code);
                        spread_flags.push(true);
                    } else {
                        self.compile_expr(argument, code);
                        spread_flags.push(false);
                    }
                }
                if spread_flags.iter().any(|is_spread| *is_spread) {
                    code.push(Opcode::ConstructWithSpread(spread_flags));
                } else {
                    code.push(Opcode::Construct(arguments.len()));
                }
            }
            Expr::Binary { op, left, right } => {
                if *op == BinaryOp::LogicalAnd {
                    self.compile_expr(left, code);
                    code.push(Opcode::Dup);
                    let jump_false_pos = code.len();
                    code.push(Opcode::JumpIfFalse(usize::MAX));
                    code.push(Opcode::Pop);
                    self.compile_expr(right, code);
                    let end = code.len();
                    code[jump_false_pos] = Opcode::JumpIfFalse(end);
                    return;
                }
                if *op == BinaryOp::LogicalOr {
                    self.compile_expr(left, code);
                    code.push(Opcode::Dup);
                    let jump_false_pos = code.len();
                    code.push(Opcode::JumpIfFalse(usize::MAX));
                    let jump_end_pos = code.len();
                    code.push(Opcode::Jump(usize::MAX));
                    let rhs_start = code.len();
                    code.push(Opcode::Pop);
                    self.compile_expr(right, code);
                    let end = code.len();
                    code[jump_false_pos] = Opcode::JumpIfFalse(rhs_start);
                    code[jump_end_pos] = Opcode::Jump(end);
                    return;
                }
                self.compile_expr(left, code);
                self.compile_expr(right, code);
                code.push(Self::binary_opcode(*op));
            }
            Expr::SpreadArgument(inner) => {
                // Spread arguments are lowered at call/construct sites.
                self.compile_expr(inner, code);
            }
        }
    }

    fn compile_update_expression(
        &mut self,
        target: &UpdateTarget,
        increment: bool,
        prefix: bool,
        code: &mut Vec<Opcode>,
    ) {
        match target {
            UpdateTarget::Identifier(Identifier(name)) => {
                code.push(Opcode::ResolveIdentifierReference(name.clone()));
                code.push(Opcode::LoadReferenceValue);
                code.push(Opcode::ToNumber);
                if !prefix {
                    code.push(Opcode::Dup);
                }
                code.push(Opcode::LoadNumber(1.0));
                code.push(if increment { Opcode::Add } else { Opcode::Sub });
                code.push(Opcode::StoreReferenceValue);
                if !prefix {
                    code.push(Opcode::Pop);
                }
            }
            UpdateTarget::Member { object, property } => {
                self.compile_expr(object, code);
                if Self::is_super_identifier(object) {
                    code.push(Opcode::LoadIdentifier("this".to_string()));
                    code.push(Opcode::Dup2);
                    code.push(Opcode::GetSuperProperty(property.clone()));
                    code.push(Opcode::ToNumber);
                    if prefix {
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::SetSuperProperty(property.clone()));
                    } else {
                        code.push(Opcode::Dup);
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::Swap);
                        code.push(Opcode::RotRight4);
                        code.push(Opcode::SetSuperProperty(property.clone()));
                        code.push(Opcode::Pop);
                    }
                } else {
                    code.push(Opcode::Dup);
                    code.push(Opcode::GetProperty(property.clone()));
                    code.push(Opcode::ToNumber);
                    if prefix {
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::SetProperty(property.clone()));
                    } else {
                        code.push(Opcode::Dup2);
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::SetProperty(property.clone()));
                        code.push(Opcode::Pop);
                        code.push(Opcode::Swap);
                        code.push(Opcode::Pop);
                    }
                }
            }
            UpdateTarget::MemberComputed { object, property } => {
                self.compile_expr(object, code);
                if Self::is_super_identifier(object) {
                    code.push(Opcode::LoadIdentifier("this".to_string()));
                    self.compile_expr(property, code);
                    code.push(Opcode::Dup3);
                    code.push(Opcode::GetSuperPropertyByValue);
                    code.push(Opcode::ToNumber);
                    if prefix {
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::SetSuperPropertyByValue);
                    } else {
                        code.push(Opcode::Dup);
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::Swap);
                        code.push(Opcode::RotRight5);
                        code.push(Opcode::SetSuperPropertyByValue);
                        code.push(Opcode::Pop);
                    }
                } else {
                    self.compile_expr(property, code);
                    code.push(Opcode::Dup2);
                    code.push(Opcode::GetPropertyByValue);
                    code.push(Opcode::ToNumber);
                    if prefix {
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::SetPropertyByValue);
                    } else {
                        code.push(Opcode::Dup);
                        code.push(Opcode::LoadNumber(1.0));
                        code.push(if increment { Opcode::Add } else { Opcode::Sub });
                        code.push(Opcode::Swap);
                        code.push(Opcode::RotRight4);
                        code.push(Opcode::SetPropertyByValue);
                        code.push(Opcode::Pop);
                    }
                }
            }
        }
    }

    fn is_compound_assign_op(op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Mod
                | BinaryOp::ShiftLeft
                | BinaryOp::ShiftRight
                | BinaryOp::UnsignedShiftRight
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
        )
    }

    fn match_identifier_compound_value<'a>(
        name: &str,
        value: &'a Expr,
    ) -> Option<(BinaryOp, &'a Expr)> {
        let Expr::Binary { op, left, right } = value else {
            return None;
        };
        if !Self::is_compound_assign_op(*op) {
            return None;
        }
        match &**left {
            Expr::Identifier(Identifier(lhs_name)) if lhs_name == name => Some((*op, right)),
            _ => None,
        }
    }

    fn match_member_compound_value<'a>(
        object: &Expr,
        property: &str,
        value: &'a Expr,
    ) -> Option<(BinaryOp, &'a Expr)> {
        let Expr::Binary { op, left, right } = value else {
            return None;
        };
        if !Self::is_compound_assign_op(*op) {
            return None;
        }
        match &**left {
            Expr::Member {
                object: left_object,
                property: left_property,
            } if **left_object == *object && left_property == property => Some((*op, right)),
            _ => None,
        }
    }

    fn match_member_computed_compound_value<'a>(
        object: &Expr,
        property: &Expr,
        value: &'a Expr,
    ) -> Option<(BinaryOp, &'a Expr)> {
        let Expr::Binary { op, left, right } = value else {
            return None;
        };
        if !Self::is_compound_assign_op(*op) {
            return None;
        }
        match &**left {
            Expr::MemberComputed {
                object: left_object,
                property: left_property,
            } if **left_object == *object && **left_property == *property => Some((*op, right)),
            _ => None,
        }
    }

    fn is_super_identifier(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Identifier(Identifier(name))
                if name == "super" || name == CLASS_CONSTRUCTOR_SUPER_BASE_BINDING
        )
    }

    fn binary_opcode(op: BinaryOp) -> Opcode {
        match op {
            BinaryOp::Add => Opcode::Add,
            BinaryOp::Sub => Opcode::Sub,
            BinaryOp::Mul => Opcode::Mul,
            BinaryOp::Div => Opcode::Div,
            BinaryOp::Mod => Opcode::Mod,
            BinaryOp::ShiftLeft => Opcode::Shl,
            BinaryOp::ShiftRight => Opcode::Shr,
            BinaryOp::UnsignedShiftRight => Opcode::UShr,
            BinaryOp::BitAnd => Opcode::BitAnd,
            BinaryOp::BitOr => Opcode::BitOr,
            BinaryOp::BitXor => Opcode::BitXor,
            BinaryOp::Equal => Opcode::Eq,
            BinaryOp::NotEqual => Opcode::Ne,
            BinaryOp::StrictEqual => Opcode::StrictEq,
            BinaryOp::StrictNotEqual => Opcode::StrictNe,
            BinaryOp::Less => Opcode::Lt,
            BinaryOp::LessEqual => Opcode::Le,
            BinaryOp::Greater => Opcode::Gt,
            BinaryOp::GreaterEqual => Opcode::Ge,
            BinaryOp::In => Opcode::In,
            BinaryOp::InstanceOf => Opcode::InstanceOf,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chunk, CompiledFunction, Opcode, compile_expression, compile_script};
    use ast::{
        BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier,
        ObjectProperty, ObjectPropertyKey, Script, Stmt, StringLiteral, SwitchCase, UnaryOp,
        UpdateTarget, VariableDeclaration,
    };

    #[test]
    fn compiles_binary_with_precedence() {
        let expr = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::Binary {
                op: BinaryOp::Mul,
                left: Box::new(Expr::Number(2.0)),
                right: Box::new(Expr::Number(3.0)),
            }),
        };

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::LoadNumber(2.0),
                Opcode::LoadNumber(3.0),
                Opcode::Mul,
                Opcode::Add,
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_sequence_expression_with_side_effect_order() {
        let expr = Expr::Sequence(vec![
            Expr::Assign {
                target: Identifier("x".to_string()),
                value: Box::new(Expr::Number(2.0)),
            },
            Expr::Number(1.0),
        ]);

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::ResolveIdentifierReference("x".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::LoadNumber(1.0),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_boolean_and_null_literals() {
        let bool_chunk = compile_expression(&Expr::Bool(true));
        assert_eq!(
            bool_chunk,
            Chunk {
                code: vec![Opcode::LoadBool(true), Opcode::Halt],
                functions: vec![],
            }
        );

        let null_chunk = compile_expression(&Expr::Null);
        assert_eq!(
            null_chunk,
            Chunk {
                code: vec![Opcode::LoadNull, Opcode::Halt],
                functions: vec![],
            }
        );

        let string_chunk = compile_expression(&Expr::String(StringLiteral {
            value: "ok".to_string(),
            has_escape: false,
        }));
        assert_eq!(
            string_chunk,
            Chunk {
                code: vec![Opcode::LoadString("ok".to_string()), Opcode::Halt],
                functions: vec![],
            }
        );
    }

    #[test]
    fn compiles_object_literal_expression() {
        let expr = Expr::ObjectLiteral(vec![
            ObjectProperty {
                key: ObjectPropertyKey::Static("answer".to_string()),
                value: Expr::Number(42.0),
            },
            ObjectProperty {
                key: ObjectPropertyKey::Static("key".to_string()),
                value: Expr::Identifier(Identifier("key".to_string())),
            },
        ]);

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::CreateObject,
                Opcode::LoadNumber(42.0),
                Opcode::DefineProperty("answer".to_string()),
                Opcode::LoadIdentifier("key".to_string()),
                Opcode::DefineProperty("key".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_object_literal_with_computed_property() {
        let expr = Expr::ObjectLiteral(vec![ObjectProperty {
            key: ObjectPropertyKey::Computed(Box::new(Expr::Identifier(Identifier(
                "k".to_string(),
            )))),
            value: Expr::Number(1.0),
        }]);

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::CreateObject,
                Opcode::Dup,
                Opcode::LoadIdentifier("k".to_string()),
                Opcode::ToPropertyKey,
                Opcode::LoadNumber(1.0),
                Opcode::SetPropertyByValue,
                Opcode::Pop,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_object_literal_with_computed_accessors() {
        let expr = Expr::ObjectLiteral(vec![
            ObjectProperty {
                key: ObjectPropertyKey::AccessorGetComputed(Box::new(Expr::Identifier(
                    Identifier("k".to_string()),
                ))),
                value: Expr::Function {
                    name: None,
                    params: vec![],
                    body: vec![Stmt::Return(Some(Expr::Number(1.0)))],
                },
            },
            ObjectProperty {
                key: ObjectPropertyKey::AccessorSetComputed(Box::new(Expr::Identifier(
                    Identifier("k".to_string()),
                ))),
                value: Expr::Function {
                    name: None,
                    params: vec![Identifier("v".to_string())],
                    body: vec![],
                },
            },
        ]);

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::CreateObject,
                Opcode::Dup,
                Opcode::LoadIdentifier("k".to_string()),
                Opcode::ToPropertyKey,
                Opcode::LoadFunction(0),
                Opcode::DefineGetterByValue,
                Opcode::Pop,
                Opcode::Dup,
                Opcode::LoadIdentifier("k".to_string()),
                Opcode::ToPropertyKey,
                Opcode::LoadFunction(1),
                Opcode::DefineSetterByValue,
                Opcode::Pop,
                Opcode::Halt,
            ],
            functions: vec![
                CompiledFunction {
                    name: "<anonymous>".to_string(),
                    length: 0,
                    params: vec![],
                    code: vec![
                        Opcode::LoadNumber(1.0),
                        Opcode::Return,
                        Opcode::LoadUndefined,
                        Opcode::Return,
                    ],
                },
                CompiledFunction {
                    name: "<anonymous>".to_string(),
                    length: 1,
                    params: vec!["v".to_string()],
                    code: vec![Opcode::LoadUndefined, Opcode::Return],
                },
            ],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_function_expression() {
        let expr = Expr::Function {
            name: None,
            params: vec![Identifier("x".to_string())],
            body: vec![Stmt::Return(Some(Expr::Identifier(Identifier(
                "x".to_string(),
            ))))],
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![Opcode::LoadFunction(0), Opcode::Halt],
            functions: vec![CompiledFunction {
                name: "<anonymous>".to_string(),
                length: 1,
                params: vec!["x".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("x".to_string()),
                    Opcode::Return,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_array_literal_expression() {
        let expr = Expr::ArrayLiteral(vec![Expr::Number(1.0), Expr::Number(2.0)]);
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::CreateArray,
                Opcode::LoadNumber(1.0),
                Opcode::ArrayAppend,
                Opcode::LoadNumber(2.0),
                Opcode::ArrayAppend,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_array_literal_with_elisions() {
        let expr = Expr::ArrayLiteral(vec![Expr::Number(1.0), Expr::Elision, Expr::Number(3.0)]);
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::CreateArray,
                Opcode::LoadNumber(1.0),
                Opcode::ArrayAppend,
                Opcode::ArrayElision,
                Opcode::LoadNumber(3.0),
                Opcode::ArrayAppend,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_array_literal_with_spread_elements() {
        let expr = Expr::ArrayLiteral(vec![
            Expr::Number(1.0),
            Expr::SpreadArgument(Box::new(Expr::Identifier(Identifier("items".to_string())))),
            Expr::Number(3.0),
        ]);
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::CreateArray,
                Opcode::LoadNumber(1.0),
                Opcode::ArrayAppend,
                Opcode::LoadIdentifier("items".to_string()),
                Opcode::ArrayAppendSpread,
                Opcode::LoadNumber(3.0),
                Opcode::ArrayAppend,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_member_assignment_expression() {
        let expr = Expr::AssignMember {
            object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
            property: "value".to_string(),
            value: Box::new(Expr::Number(1.0)),
        };

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadIdentifier("obj".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::SetProperty("value".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_computed_member_assignment_expression() {
        let expr = Expr::AssignMemberComputed {
            object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
            property: Box::new(Expr::Identifier(Identifier("key".to_string()))),
            value: Box::new(Expr::Number(1.0)),
        };

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadIdentifier("obj".to_string()),
                Opcode::LoadIdentifier("key".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::SetPropertyByValue,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_prefix_update_expression() {
        let expr = Expr::Update {
            target: UpdateTarget::Identifier(Identifier("x".to_string())),
            increment: true,
            prefix: true,
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::ResolveIdentifierReference("x".to_string()),
                Opcode::LoadReferenceValue,
                Opcode::ToNumber,
                Opcode::LoadNumber(1.0),
                Opcode::Add,
                Opcode::StoreReferenceValue,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_postfix_computed_update_expression() {
        let expr = Expr::Update {
            target: UpdateTarget::MemberComputed {
                object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
                property: Box::new(Expr::Identifier(Identifier("key".to_string()))),
            },
            increment: true,
            prefix: false,
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadIdentifier("obj".to_string()),
                Opcode::LoadIdentifier("key".to_string()),
                Opcode::Dup2,
                Opcode::GetPropertyByValue,
                Opcode::ToNumber,
                Opcode::Dup,
                Opcode::LoadNumber(1.0),
                Opcode::Add,
                Opcode::Swap,
                Opcode::RotRight4,
                Opcode::SetPropertyByValue,
                Opcode::Pop,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_script_with_bindings() {
        let script = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("x".to_string()),
                    initializer: Some(Expr::Number(1.0)),
                }),
                Stmt::Expression(Expr::Assign {
                    target: Identifier("x".to_string()),
                    value: Box::new(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                        right: Box::new(Expr::Number(2.0)),
                    }),
                }),
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUninitialized,
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::ResolveIdentifierReference("x".to_string()),
                Opcode::LoadReferenceValue,
                Opcode::LoadNumber(2.0),
                Opcode::Add,
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_var_declaration_as_mutable_binding() {
        let script = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Var,
                    name: Identifier("x".to_string()),
                    initializer: None,
                }),
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };
        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::DefineVar("x".to_string()),
                Opcode::Nop,
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_block_scope_with_shadowing() {
        let script = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("x".to_string()),
                    initializer: Some(Expr::Number(1.0)),
                }),
                Stmt::Block(vec![
                    Stmt::VariableDeclaration(VariableDeclaration {
                        kind: BindingKind::Let,
                        name: Identifier("x".to_string()),
                        initializer: Some(Expr::Number(2.0)),
                    }),
                    Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
                ]),
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUninitialized,
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::EnterScope,
                Opcode::LoadUninitialized,
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadNumber(2.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_function_declaration_and_call() {
        let script = Script {
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

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "add".to_string(),
                    function_id: 0,
                },
                Opcode::Nop,
                Opcode::LoadNumber(1.0),
                Opcode::LoadNumber(2.0),
                Opcode::CallIdentifier {
                    name: "add".to_string(),
                    arg_count: 2,
                },
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "add".to_string(),
                length: 2,
                params: vec!["a".to_string(), "b".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("a".to_string()),
                    Opcode::LoadIdentifier("b".to_string()),
                    Opcode::Add,
                    Opcode::Return,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn hoists_function_declaration_before_use() {
        let script = Script {
            statements: vec![
                Stmt::Expression(Expr::Call {
                    callee: Box::new(Expr::Identifier(Identifier("id".to_string()))),
                    arguments: vec![Expr::Number(42.0)],
                }),
                Stmt::FunctionDeclaration(FunctionDeclaration {
                    name: Identifier("id".to_string()),
                    params: vec![Identifier("x".to_string())],
                    body: vec![Stmt::Return(Some(Expr::Identifier(Identifier(
                        "x".to_string(),
                    ))))],
                }),
            ],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "id".to_string(),
                    function_id: 0,
                },
                Opcode::LoadNumber(42.0),
                Opcode::CallIdentifier {
                    name: "id".to_string(),
                    arg_count: 1,
                },
                Opcode::Nop,
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "id".to_string(),
                length: 1,
                params: vec!["x".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("x".to_string()),
                    Opcode::Return,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_member_call_with_receiver_binding() {
        let script = Script {
            statements: vec![Stmt::Expression(Expr::Call {
                callee: Box::new(Expr::Member {
                    object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
                    property: "m".to_string(),
                }),
                arguments: vec![Expr::Number(1.0)],
            })],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadIdentifier("obj".to_string()),
                Opcode::Dup,
                Opcode::GetProperty("m".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::CallMethod(1),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_if_else_statement() {
        let script = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("x".to_string()),
                    initializer: Some(Expr::Number(0.0)),
                }),
                Stmt::If {
                    condition: Expr::Binary {
                        op: BinaryOp::Less,
                        left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                        right: Box::new(Expr::Number(1.0)),
                    },
                    consequent: Box::new(Stmt::Expression(Expr::Assign {
                        target: Identifier("x".to_string()),
                        value: Box::new(Expr::Number(1.0)),
                    })),
                    alternate: Some(Box::new(Stmt::Expression(Expr::Assign {
                        target: Identifier("x".to_string()),
                        value: Box::new(Expr::Number(2.0)),
                    }))),
                },
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUninitialized,
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadNumber(0.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Lt,
                Opcode::JumpIfFalse(13),
                Opcode::ResolveIdentifierReference("x".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::Jump(17),
                Opcode::ResolveIdentifierReference("x".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_while_statement() {
        let script = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("x".to_string()),
                    initializer: Some(Expr::Number(0.0)),
                }),
                Stmt::While {
                    condition: Expr::Binary {
                        op: BinaryOp::Less,
                        left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                        right: Box::new(Expr::Number(3.0)),
                    },
                    body: Box::new(Stmt::Expression(Expr::Assign {
                        target: Identifier("x".to_string()),
                        value: Box::new(Expr::Binary {
                            op: BinaryOp::Add,
                            left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                            right: Box::new(Expr::Number(1.0)),
                        }),
                    })),
                },
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUninitialized,
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadNumber(0.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::LoadNumber(3.0),
                Opcode::Lt,
                Opcode::JumpIfFalse(15),
                Opcode::ResolveIdentifierReference("x".to_string()),
                Opcode::LoadReferenceValue,
                Opcode::LoadNumber(1.0),
                Opcode::Add,
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::Jump(4),
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_do_while_statement() {
        let script = Script {
            statements: vec![Stmt::DoWhile {
                body: Box::new(Stmt::Expression(Expr::Number(1.0))),
                condition: Expr::Number(0.0),
            }],
        };
        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_0".to_string(),
                    mutable: true,
                },
                Opcode::LoadUndefined,
                Opcode::StoreVariable("$__loop_completion_0".to_string()),
                Opcode::Pop,
                Opcode::LoadNumber(1.0),
                Opcode::StoreVariable("$__loop_completion_0".to_string()),
                Opcode::Pop,
                Opcode::LoadNumber(0.0),
                Opcode::JumpIfFalse(11),
                Opcode::Jump(2),
                Opcode::LoadIdentifier("$__loop_completion_0".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn keeps_statement_value_for_terminal_if_without_else() {
        let script = Script {
            statements: vec![Stmt::If {
                condition: Expr::Number(0.0),
                consequent: Box::new(Stmt::Expression(Expr::Number(1.0))),
                alternate: None,
            }],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadNumber(0.0),
                Opcode::JumpIfFalse(4),
                Opcode::LoadNumber(1.0),
                Opcode::Jump(5),
                Opcode::LoadUndefined,
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_for_statement() {
        let script = Script {
            statements: vec![Stmt::For {
                initializer: Some(ForInitializer::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("i".to_string()),
                    initializer: Some(Expr::Number(0.0)),
                })),
                condition: Some(Expr::Binary {
                    op: BinaryOp::Less,
                    left: Box::new(Expr::Identifier(Identifier("i".to_string()))),
                    right: Box::new(Expr::Number(2.0)),
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

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::EnterScope,
                Opcode::LoadNumber(0.0),
                Opcode::DefineVariable {
                    name: "i".to_string(),
                    mutable: true,
                },
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_0".to_string(),
                    mutable: true,
                },
                Opcode::ResolveIdentifierReference("$__loop_completion_0".to_string()),
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_1".to_string(),
                    mutable: true,
                },
                Opcode::EnterScope,
                Opcode::LoadIdentifier("$__loop_completion_0".to_string()),
                Opcode::DefineVariable {
                    name: "i".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::Lt,
                Opcode::JumpIfFalse(44),
                Opcode::LoadUndefined,
                Opcode::StoreVariable("$__loop_completion_1".to_string()),
                Opcode::Pop,
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::StoreVariable("$__loop_completion_1".to_string()),
                Opcode::Pop,
                Opcode::ResolveIdentifierReference("$__loop_completion_0".to_string()),
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::EnterScope,
                Opcode::LoadIdentifier("$__loop_completion_0".to_string()),
                Opcode::DefineVariable {
                    name: "i".to_string(),
                    mutable: true,
                },
                Opcode::ResolveIdentifierReference("i".to_string()),
                Opcode::LoadReferenceValue,
                Opcode::LoadNumber(1.0),
                Opcode::Add,
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::ResolveIdentifierReference("$__loop_completion_0".to_string()),
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::StoreReferenceValue,
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::Jump(11),
                Opcode::ExitScope,
                Opcode::Jump(46),
                Opcode::LoadIdentifier("$__loop_completion_1".to_string()),
                Opcode::ExitScope,
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_loop_control_with_scope_cleanup() {
        let script = Script {
            statements: vec![Stmt::While {
                condition: Expr::Number(1.0),
                body: Box::new(Stmt::Block(vec![
                    Stmt::Block(vec![Stmt::Continue]),
                    Stmt::Break,
                ])),
            }],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_0".to_string(),
                    mutable: true,
                },
                Opcode::LoadNumber(1.0),
                Opcode::JumpIfFalse(17),
                Opcode::LoadUndefined,
                Opcode::StoreVariable("$__loop_completion_0".to_string()),
                Opcode::Pop,
                Opcode::EnterScope,
                Opcode::EnterScope,
                Opcode::ExitScope,
                Opcode::ExitScope,
                Opcode::Jump(2),
                Opcode::ExitScope,
                Opcode::ExitScope,
                Opcode::Jump(17),
                Opcode::ExitScope,
                Opcode::Jump(2),
                Opcode::LoadIdentifier("$__loop_completion_0".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_switch_statement() {
        let script = Script {
            statements: vec![Stmt::Switch {
                discriminant: Expr::Identifier(Identifier("x".to_string())),
                cases: vec![
                    SwitchCase {
                        test: Some(Expr::Number(1.0)),
                        consequent: vec![
                            Stmt::Expression(Expr::Assign {
                                target: Identifier("y".to_string()),
                                value: Box::new(Expr::Number(1.0)),
                            }),
                            Stmt::Break,
                        ],
                    },
                    SwitchCase {
                        test: None,
                        consequent: vec![Stmt::Expression(Expr::Assign {
                            target: Identifier("y".to_string()),
                            value: Box::new(Expr::Number(2.0)),
                        })],
                    },
                ],
            }],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_1".to_string(),
                    mutable: true,
                },
                Opcode::EnterScope,
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::DefineVariable {
                    name: "$__switch_tmp_0".to_string(),
                    mutable: false,
                },
                Opcode::LoadIdentifier("$__switch_tmp_0".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::StrictEq,
                Opcode::JumpIfFalse(10),
                Opcode::Jump(11),
                Opcode::Jump(17),
                Opcode::ResolveIdentifierReference("y".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::StoreReferenceValue,
                Opcode::StoreVariable("$__loop_completion_1".to_string()),
                Opcode::Pop,
                Opcode::Jump(22),
                Opcode::ResolveIdentifierReference("y".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::StoreReferenceValue,
                Opcode::StoreVariable("$__loop_completion_1".to_string()),
                Opcode::Pop,
                Opcode::LoadIdentifier("$__loop_completion_1".to_string()),
                Opcode::ExitScope,
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_switch_break_with_scope_cleanup() {
        let script = Script {
            statements: vec![Stmt::Switch {
                discriminant: Expr::Number(1.0),
                cases: vec![SwitchCase {
                    test: Some(Expr::Number(1.0)),
                    consequent: vec![Stmt::Block(vec![Stmt::Break])],
                }],
            }],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_1".to_string(),
                    mutable: true,
                },
                Opcode::EnterScope,
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "$__switch_tmp_0".to_string(),
                    mutable: false,
                },
                Opcode::LoadIdentifier("$__switch_tmp_0".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::StrictEq,
                Opcode::JumpIfFalse(10),
                Opcode::Jump(11),
                Opcode::Jump(15),
                Opcode::EnterScope,
                Opcode::ExitScope,
                Opcode::Jump(15),
                Opcode::ExitScope,
                Opcode::LoadIdentifier("$__loop_completion_1".to_string()),
                Opcode::ExitScope,
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_try_catch_statement() {
        let script = Script {
            statements: vec![Stmt::Try {
                try_block: vec![Stmt::Throw(Expr::Number(1.0))],
                catch_param: Some(Identifier("e".to_string())),
                catch_block: Some(vec![Stmt::Expression(Expr::Identifier(Identifier(
                    "e".to_string(),
                )))]),
                finally_block: None,
            }],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_0".to_string(),
                    mutable: true,
                },
                Opcode::PushExceptionHandler {
                    catch_target: Some(11),
                    finally_target: None,
                },
                Opcode::EnterScope,
                Opcode::LoadNumber(1.0),
                Opcode::Throw,
                Opcode::StoreVariable("$__loop_completion_0".to_string()),
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::PopExceptionHandler,
                Opcode::Jump(18),
                Opcode::EnterScope,
                Opcode::LoadException,
                Opcode::DefineVariable {
                    name: "e".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("e".to_string()),
                Opcode::StoreVariable("$__loop_completion_0".to_string()),
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::LoadIdentifier("$__loop_completion_0".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_try_finally_statement() {
        let script = Script {
            statements: vec![Stmt::Try {
                try_block: vec![Stmt::Expression(Expr::Number(1.0))],
                catch_param: None,
                catch_block: None,
                finally_block: Some(vec![Stmt::Expression(Expr::Number(2.0))]),
            }],
        };

        let chunk = compile_script(&script);
        let expected = Chunk {
            code: vec![
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "$__loop_completion_0".to_string(),
                    mutable: true,
                },
                Opcode::PushExceptionHandler {
                    catch_target: None,
                    finally_target: Some(10),
                },
                Opcode::EnterScope,
                Opcode::LoadNumber(1.0),
                Opcode::StoreVariable("$__loop_completion_0".to_string()),
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::PopExceptionHandler,
                Opcode::Jump(10),
                Opcode::EnterScope,
                Opcode::LoadNumber(2.0),
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::RethrowIfException,
                Opcode::LoadIdentifier("$__loop_completion_0".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };

        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_unary_and_comparison_ops() {
        let expr = Expr::Binary {
            op: BinaryOp::GreaterEqual,
            left: Box::new(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(Expr::Number(2.0)),
            }),
            right: Box::new(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(Expr::Number(-2.0)),
            }),
        };

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadNumber(2.0),
                Opcode::Neg,
                Opcode::LoadNumber(-2.0),
                Opcode::LoadNumber(0.0),
                Opcode::Sub,
                Opcode::Ge,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_in_operator() {
        let expr = Expr::Binary {
            op: BinaryOp::In,
            left: Box::new(Expr::String(StringLiteral {
                value: "x".to_string(),
                has_escape: false,
            })),
            right: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadString("x".to_string()),
                Opcode::LoadIdentifier("obj".to_string()),
                Opcode::In,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_instanceof_operator() {
        let expr = Expr::Binary {
            op: BinaryOp::InstanceOf,
            left: Box::new(Expr::Identifier(Identifier("value".to_string()))),
            right: Box::new(Expr::Identifier(Identifier("Ctor".to_string()))),
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadIdentifier("value".to_string()),
                Opcode::LoadIdentifier("Ctor".to_string()),
                Opcode::InstanceOf,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_typeof_void_and_delete_unary_ops() {
        let typeof_ident = compile_expression(&Expr::Unary {
            op: UnaryOp::Typeof,
            expr: Box::new(Expr::Identifier(Identifier("x".to_string()))),
        });
        assert_eq!(
            typeof_ident,
            Chunk {
                code: vec![Opcode::TypeofIdentifier("x".to_string()), Opcode::Halt],
                functions: vec![],
            }
        );

        let void_expr = compile_expression(&Expr::Unary {
            op: UnaryOp::Void,
            expr: Box::new(Expr::Number(1.0)),
        });
        assert_eq!(
            void_expr,
            Chunk {
                code: vec![
                    Opcode::LoadNumber(1.0),
                    Opcode::Pop,
                    Opcode::LoadUndefined,
                    Opcode::Halt
                ],
                functions: vec![],
            }
        );

        let delete_expr = compile_expression(&Expr::Unary {
            op: UnaryOp::Delete,
            expr: Box::new(Expr::Identifier(Identifier("x".to_string()))),
        });
        assert_eq!(
            delete_expr,
            Chunk {
                code: vec![Opcode::DeleteIdentifier("x".to_string()), Opcode::Halt],
                functions: vec![],
            }
        );

        let delete_member = compile_expression(&Expr::Unary {
            op: UnaryOp::Delete,
            expr: Box::new(Expr::Member {
                object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
                property: "x".to_string(),
            }),
        });
        assert_eq!(
            delete_member,
            Chunk {
                code: vec![
                    Opcode::LoadIdentifier("obj".to_string()),
                    Opcode::DeleteProperty("x".to_string()),
                    Opcode::Halt,
                ],
                functions: vec![],
            }
        );

        let delete_member_computed = compile_expression(&Expr::Unary {
            op: UnaryOp::Delete,
            expr: Box::new(Expr::MemberComputed {
                object: Box::new(Expr::Identifier(Identifier("obj".to_string()))),
                property: Box::new(Expr::Identifier(Identifier("key".to_string()))),
            }),
        });
        assert_eq!(
            delete_member_computed,
            Chunk {
                code: vec![
                    Opcode::LoadIdentifier("obj".to_string()),
                    Opcode::LoadIdentifier("key".to_string()),
                    Opcode::DeletePropertyByValue,
                    Opcode::Halt,
                ],
                functions: vec![],
            }
        );
    }

    #[test]
    fn compiles_strict_equality_ops() {
        let expr = Expr::Binary {
            op: BinaryOp::StrictNotEqual,
            left: Box::new(Expr::Binary {
                op: BinaryOp::StrictEqual,
                left: Box::new(Expr::Number(1.0)),
                right: Box::new(Expr::Number(1.0)),
            }),
            right: Box::new(Expr::Number(0.0)),
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::LoadNumber(1.0),
                Opcode::StrictEq,
                Opcode::LoadNumber(0.0),
                Opcode::StrictNe,
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_logical_and_with_short_circuit() {
        let expr = Expr::Binary {
            op: BinaryOp::LogicalAnd,
            left: Box::new(Expr::Identifier(Identifier("a".to_string()))),
            right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadIdentifier("a".to_string()),
                Opcode::Dup,
                Opcode::JumpIfFalse(5),
                Opcode::Pop,
                Opcode::LoadIdentifier("b".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }

    #[test]
    fn compiles_logical_or_with_short_circuit() {
        let expr = Expr::Binary {
            op: BinaryOp::LogicalOr,
            left: Box::new(Expr::Identifier(Identifier("a".to_string()))),
            right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
        };
        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadIdentifier("a".to_string()),
                Opcode::Dup,
                Opcode::JumpIfFalse(4),
                Opcode::Jump(6),
                Opcode::Pop,
                Opcode::LoadIdentifier("b".to_string()),
                Opcode::Halt,
            ],
            functions: vec![],
        };
        assert_eq!(chunk, expected);
    }
}
