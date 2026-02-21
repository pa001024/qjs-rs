#![forbid(unsafe_code)]

use ast::{
    BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier,
    ObjectPropertyKey, Script, Stmt, SwitchCase, UnaryOp, VariableDeclaration,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    LoadNumber(f64),
    LoadBool(bool),
    LoadNull,
    LoadString(String),
    LoadUndefined,
    CreateObject,
    LoadIdentifier(String),
    LoadFunction(usize),
    DefineVariable {
        name: String,
        mutable: bool,
    },
    DefineFunction {
        name: String,
        function_id: usize,
    },
    StoreVariable(String),
    GetProperty(String),
    GetPropertyByValue,
    DefineProperty(String),
    DefineGetter(String),
    DefineSetter(String),
    DefineGetterByValue,
    DefineSetterByValue,
    SetProperty(String),
    SetPropertyByValue,
    DeleteIdentifier(String),
    DeleteProperty(String),
    DeletePropertyByValue,
    EnterScope,
    ExitScope,
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
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
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
    Construct(usize),
    ConstructWithSpread(Vec<bool>),
    Return,
    Dup,
    Pop,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompiledFunction {
    pub name: String,
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

#[derive(Debug, Clone, PartialEq, Default)]
struct FinallyContext {
    handler_depth: usize,
    finally_block: Vec<Stmt>,
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
        if self.scope_depth == 0 && self.function_nesting == 0 {
            let mut hoisted_var_names = BTreeSet::new();
            self.collect_hoisted_var_names(statements, &mut hoisted_var_names);
            for name in hoisted_var_names {
                code.push(Opcode::LoadUndefined);
                code.push(Opcode::DefineVariable {
                    name,
                    mutable: true,
                });
            }
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

        let mut produced_value = false;
        let last_executable = executable_indexes.last().copied();
        for (index, stmt) in statements.iter().enumerate() {
            if matches!(stmt, Stmt::FunctionDeclaration(_)) {
                continue;
            }
            let keep_value = preserve_value && Some(index) == last_executable;
            produced_value = self.compile_stmt(stmt, code, keep_value);
        }

        produced_value
    }

    fn collect_hoisted_var_names(&self, statements: &[Stmt], names: &mut BTreeSet<String>) {
        for stmt in statements {
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
                Stmt::For {
                    initializer: Some(initializer),
                    ..
                } => match initializer {
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
                },
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
                if *kind == BindingKind::Var
                    && self.scope_depth == 0
                    && self.function_nesting == 0
                    && initializer.is_none()
                {
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
                self.compile_expr(expr, code);
                if !keep_value {
                    code.push(Opcode::Pop);
                    false
                } else {
                    true
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
                let loop_start = code.len();
                self.compile_expr(condition, code);
                let jump_to_end_pos = code.len();
                code.push(Opcode::JumpIfFalse(usize::MAX));
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

                self.compile_stmt(body, code, false);
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

                if keep_value {
                    code.push(Opcode::LoadUndefined);
                    true
                } else {
                    false
                }
            }
            Stmt::DoWhile { body, condition } => {
                let loop_start = code.len();
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

                self.compile_stmt(body, code, false);
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

                if keep_value {
                    code.push(Opcode::LoadUndefined);
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

                let loop_start = code.len();
                let jump_to_end_pos = if let Some(condition) = condition {
                    self.compile_expr(condition, code);
                    let jump_to_end_pos = code.len();
                    code.push(Opcode::JumpIfFalse(usize::MAX));
                    Some(jump_to_end_pos)
                } else {
                    None
                };

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

                self.compile_stmt(body, code, false);
                let continue_target = code.len();
                if let Some(update) = update {
                    self.compile_expr(update, code);
                    code.push(Opcode::Pop);
                }
                code.push(Opcode::Jump(loop_start));

                let loop_end = code.len();
                if let Some(jump_to_end_pos) = jump_to_end_pos {
                    code[jump_to_end_pos] = Opcode::JumpIfFalse(loop_end);
                }
                let break_context = self
                    .break_contexts
                    .pop()
                    .expect("loop break context should exist");
                let loop_context = self.loops.pop().expect("loop context should exist");
                self.patch_loop_exits(loop_context, continue_target, code);
                self.patch_break_exits(break_context, loop_end, code);

                code.push(Opcode::ExitScope);
                self.scope_depth = self.scope_depth.saturating_sub(1);

                if keep_value {
                    code.push(Opcode::LoadUndefined);
                    true
                } else {
                    false
                }
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                let temp_name = self.next_switch_temp_name();
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
                        code.push(Opcode::Eq);

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
                for (index, SwitchCase { consequent, .. }) in cases.iter().enumerate() {
                    case_start_positions[index] = code.len();
                    self.compile_statement_list(consequent, code, false);
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

                code.push(Opcode::ExitScope);
                self.scope_depth = self.scope_depth.saturating_sub(1);

                if keep_value {
                    code.push(Opcode::LoadUndefined);
                    true
                } else {
                    false
                }
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

                let body_value = self.compile_stmt(body, code, keep_value);
                let label_end = code.len();
                self.label_contexts.pop();
                let break_context = self
                    .break_contexts
                    .pop()
                    .expect("label break context should exist");
                self.patch_break_exits(break_context, label_end, code);
                body_value
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
        let handler_pos = code.len();
        code.push(Opcode::PushExceptionHandler {
            catch_target: None,
            finally_target: None,
        });
        self.handler_depth += 1;

        self.compile_statement_list(try_block, code, false);
        code.push(Opcode::PopExceptionHandler);
        self.handler_depth = self.handler_depth.saturating_sub(1);

        let jump_after_catch_pos = code.len();
        code.push(Opcode::Jump(usize::MAX));
        let catch_start = code.len();
        code[handler_pos] = Opcode::PushExceptionHandler {
            catch_target: Some(catch_start),
            finally_target: None,
        };

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

        self.compile_statement_list(catch_block, code, false);
        let end = code.len();
        code[jump_after_catch_pos] = Opcode::Jump(end);

        if keep_value {
            code.push(Opcode::LoadUndefined);
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
        let handler_pos = code.len();
        code.push(Opcode::PushExceptionHandler {
            catch_target: None,
            finally_target: None,
        });
        self.handler_depth += 1;
        self.finally_contexts.push(FinallyContext {
            handler_depth: self.handler_depth,
            finally_block: finally_block.to_vec(),
        });

        self.compile_statement_list(try_block, code, false);
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

        self.compile_statement_list(finally_block, code, false);
        code.push(Opcode::RethrowIfException);

        if keep_value {
            code.push(Opcode::LoadUndefined);
            true
        } else {
            false
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
        let saved_function_nesting = self.function_nesting;
        self.scope_depth = 0;
        self.handler_depth = 0;
        self.function_nesting = self.function_nesting.saturating_add(1);

        let mut code = Vec::new();
        self.compile_statement_list(body, &mut code, false);
        code.push(Opcode::LoadUndefined);
        code.push(Opcode::Return);

        let function_id = self.functions.len();
        self.functions.push(CompiledFunction {
            name: name
                .map(|identifier| identifier.0.clone())
                .unwrap_or_else(|| "<anonymous>".to_string()),
            params: params.iter().map(|param| param.0.clone()).collect(),
            code,
        });

        self.scope_depth = saved_scope_depth;
        self.handler_depth = saved_handler_depth;
        self.loops = saved_loops;
        self.break_contexts = saved_break_contexts;
        self.label_contexts = saved_label_contexts;
        self.finally_contexts = saved_finally_contexts;
        self.function_nesting = saved_function_nesting;
        function_id
    }

    fn statement_allows_continue_label(statement: &Stmt) -> bool {
        match statement {
            Stmt::While { .. } | Stmt::DoWhile { .. } | Stmt::For { .. } => true,
            Stmt::Labeled { body, .. } => Self::statement_allows_continue_label(body),
            _ => false,
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
            code.push(Opcode::PopExceptionHandler);
            self.handler_depth = self.handler_depth.saturating_sub(1);
            self.compile_statement_list(&context.finally_block, code, false);
        }
    }

    fn next_switch_temp_name(&mut self) -> String {
        let id = self.next_switch_temp_id;
        self.next_switch_temp_id += 1;
        format!("$__switch_tmp_{id}")
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
            Expr::String(value) => code.push(Opcode::LoadString(value.clone())),
            Expr::RegexLiteral { pattern, flags } => {
                code.push(Opcode::CreateObject);
                code.push(Opcode::LoadString(pattern.clone()));
                code.push(Opcode::DefineProperty("source".to_string()));
                code.push(Opcode::LoadString(flags.clone()));
                code.push(Opcode::DefineProperty("flags".to_string()));
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
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::SetPropertyByValue);
                            code.push(Opcode::Pop);
                        }
                        ObjectPropertyKey::AccessorGetComputed(key_expr) => {
                            code.push(Opcode::Dup);
                            self.compile_expr(key_expr, code);
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineGetterByValue);
                            code.push(Opcode::Pop);
                        }
                        ObjectPropertyKey::AccessorSetComputed(key_expr) => {
                            code.push(Opcode::Dup);
                            self.compile_expr(key_expr, code);
                            self.compile_expr(&property.value, code);
                            code.push(Opcode::DefineSetterByValue);
                            code.push(Opcode::Pop);
                        }
                    }
                }
            }
            Expr::ArrayLiteral(elements) => {
                code.push(Opcode::CreateObject);
                for (index, element) in elements.iter().enumerate() {
                    self.compile_expr(element, code);
                    code.push(Opcode::DefineProperty(index.to_string()));
                }
                code.push(Opcode::LoadNumber(elements.len() as f64));
                code.push(Opcode::DefineProperty("length".to_string()));
            }
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
                            self.compile_expr(object, code);
                            code.push(Opcode::DeleteProperty(property.clone()));
                        }
                        Expr::MemberComputed { object, property } => {
                            self.compile_expr(object, code);
                            self.compile_expr(property, code);
                            code.push(Opcode::DeletePropertyByValue);
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
                    UnaryOp::Plus => return,
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
            Expr::Assign {
                target: Identifier(name),
                value,
            } => {
                self.compile_expr(value, code);
                code.push(Opcode::StoreVariable(name.clone()));
            }
            Expr::AssignMember {
                object,
                property,
                value,
            } => {
                self.compile_expr(object, code);
                self.compile_expr(value, code);
                code.push(Opcode::SetProperty(property.clone()));
            }
            Expr::AssignMemberComputed {
                object,
                property,
                value,
            } => {
                self.compile_expr(object, code);
                self.compile_expr(property, code);
                self.compile_expr(value, code);
                code.push(Opcode::SetPropertyByValue);
            }
            Expr::Identifier(Identifier(name)) => code.push(Opcode::LoadIdentifier(name.clone())),
            Expr::Member { object, property } => {
                self.compile_expr(object, code);
                code.push(Opcode::GetProperty(property.clone()));
            }
            Expr::MemberComputed { object, property } => {
                self.compile_expr(object, code);
                self.compile_expr(property, code);
                code.push(Opcode::GetPropertyByValue);
            }
            Expr::Call { callee, arguments } => {
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
                let opcode = match op {
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
                    BinaryOp::StrictEqual => Opcode::Eq,
                    BinaryOp::StrictNotEqual => Opcode::Ne,
                    BinaryOp::Less => Opcode::Lt,
                    BinaryOp::LessEqual => Opcode::Le,
                    BinaryOp::Greater => Opcode::Gt,
                    BinaryOp::GreaterEqual => Opcode::Ge,
                    BinaryOp::In => Opcode::In,
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => unreachable!(),
                };
                code.push(opcode);
            }
            Expr::SpreadArgument(inner) => {
                // Spread arguments are lowered at call/construct sites.
                self.compile_expr(inner, code);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chunk, CompiledFunction, Opcode, compile_expression, compile_script};
    use ast::{
        BinaryOp, BindingKind, Expr, ForInitializer, FunctionDeclaration, Identifier,
        ObjectProperty, ObjectPropertyKey, Script, Stmt, SwitchCase, UnaryOp, VariableDeclaration,
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

        let string_chunk = compile_expression(&Expr::String("ok".to_string()));
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
                Opcode::LoadFunction(0),
                Opcode::DefineGetterByValue,
                Opcode::Pop,
                Opcode::Dup,
                Opcode::LoadIdentifier("k".to_string()),
                Opcode::LoadFunction(1),
                Opcode::DefineSetterByValue,
                Opcode::Pop,
                Opcode::Halt,
            ],
            functions: vec![
                CompiledFunction {
                    name: "<anonymous>".to_string(),
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
                Opcode::CreateObject,
                Opcode::LoadNumber(1.0),
                Opcode::DefineProperty("0".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::DefineProperty("1".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::DefineProperty("length".to_string()),
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
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::Add,
                Opcode::StoreVariable("x".to_string()),
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
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
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
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::EnterScope,
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
                Opcode::LoadIdentifier("add".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::LoadNumber(2.0),
                Opcode::Call(2),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "add".to_string(),
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
                Opcode::LoadIdentifier("id".to_string()),
                Opcode::LoadNumber(42.0),
                Opcode::Call(1),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "id".to_string(),
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
                Opcode::LoadNumber(0.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Lt,
                Opcode::JumpIfFalse(10),
                Opcode::LoadNumber(1.0),
                Opcode::StoreVariable("x".to_string()),
                Opcode::Pop,
                Opcode::Jump(13),
                Opcode::LoadNumber(2.0),
                Opcode::StoreVariable("x".to_string()),
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
                Opcode::LoadNumber(0.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::LoadNumber(3.0),
                Opcode::Lt,
                Opcode::JumpIfFalse(12),
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Add,
                Opcode::StoreVariable("x".to_string()),
                Opcode::Pop,
                Opcode::Jump(2),
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
                Opcode::LoadNumber(1.0),
                Opcode::Pop,
                Opcode::LoadNumber(0.0),
                Opcode::JumpIfFalse(5),
                Opcode::Jump(0),
                Opcode::LoadUndefined,
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
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::Lt,
                Opcode::JumpIfFalse(15),
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::Pop,
                Opcode::LoadIdentifier("i".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Add,
                Opcode::StoreVariable("i".to_string()),
                Opcode::Pop,
                Opcode::Jump(3),
                Opcode::ExitScope,
                Opcode::LoadUndefined,
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
                Opcode::LoadNumber(1.0),
                Opcode::JumpIfFalse(12),
                Opcode::EnterScope,
                Opcode::EnterScope,
                Opcode::ExitScope,
                Opcode::ExitScope,
                Opcode::Jump(0),
                Opcode::ExitScope,
                Opcode::ExitScope,
                Opcode::Jump(12),
                Opcode::ExitScope,
                Opcode::Jump(0),
                Opcode::LoadUndefined,
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
                Opcode::EnterScope,
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::DefineVariable {
                    name: "$__switch_tmp_0".to_string(),
                    mutable: false,
                },
                Opcode::LoadIdentifier("$__switch_tmp_0".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Eq,
                Opcode::JumpIfFalse(8),
                Opcode::Jump(9),
                Opcode::Jump(13),
                Opcode::LoadNumber(1.0),
                Opcode::StoreVariable("y".to_string()),
                Opcode::Pop,
                Opcode::Jump(16),
                Opcode::LoadNumber(2.0),
                Opcode::StoreVariable("y".to_string()),
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::LoadUndefined,
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
                Opcode::EnterScope,
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "$__switch_tmp_0".to_string(),
                    mutable: false,
                },
                Opcode::LoadIdentifier("$__switch_tmp_0".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Eq,
                Opcode::JumpIfFalse(8),
                Opcode::Jump(9),
                Opcode::Jump(13),
                Opcode::EnterScope,
                Opcode::ExitScope,
                Opcode::Jump(13),
                Opcode::ExitScope,
                Opcode::ExitScope,
                Opcode::LoadUndefined,
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
                Opcode::PushExceptionHandler {
                    catch_target: Some(5),
                    finally_target: None,
                },
                Opcode::LoadNumber(1.0),
                Opcode::Throw,
                Opcode::PopExceptionHandler,
                Opcode::Jump(9),
                Opcode::LoadException,
                Opcode::DefineVariable {
                    name: "e".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("e".to_string()),
                Opcode::Pop,
                Opcode::LoadUndefined,
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
                Opcode::PushExceptionHandler {
                    catch_target: None,
                    finally_target: Some(5),
                },
                Opcode::LoadNumber(1.0),
                Opcode::Pop,
                Opcode::PopExceptionHandler,
                Opcode::Jump(5),
                Opcode::LoadNumber(2.0),
                Opcode::Pop,
                Opcode::RethrowIfException,
                Opcode::LoadUndefined,
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
            left: Box::new(Expr::String("x".to_string())),
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
                Opcode::Eq,
                Opcode::LoadNumber(0.0),
                Opcode::Ne,
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
