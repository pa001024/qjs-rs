#![forbid(unsafe_code)]

use ast::{
    BinaryOp, BindingKind, Expr, FunctionDeclaration, Identifier, Script, Stmt, UnaryOp,
    VariableDeclaration,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    LoadNumber(f64),
    LoadUndefined,
    LoadIdentifier(String),
    DefineVariable { name: String, mutable: bool },
    DefineFunction { name: String, function_id: usize },
    StoreVariable(String),
    EnterScope,
    ExitScope,
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    JumpIfFalse(usize),
    Jump(usize),
    Call(usize),
    Return,
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
        // Function declarations are hoisted to the top of their containing scope.
        for stmt in statements {
            if let Stmt::FunctionDeclaration(FunctionDeclaration { name, params, body }) = stmt {
                let function_id = self.compile_function(name, params, body);
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

    fn compile_stmt(&mut self, stmt: &Stmt, code: &mut Vec<Opcode>, keep_value: bool) -> bool {
        match stmt {
            Stmt::VariableDeclaration(VariableDeclaration {
                kind,
                name: Identifier(binding_name),
                initializer,
            }) => {
                if let Some(expr) = initializer {
                    self.compile_expr(expr, code);
                } else {
                    code.push(Opcode::LoadUndefined);
                }
                code.push(Opcode::DefineVariable {
                    name: binding_name.clone(),
                    mutable: matches!(kind, BindingKind::Let),
                });
                false
            }
            Stmt::FunctionDeclaration(FunctionDeclaration { name, params, body }) => {
                let function_id = self.compile_function(name, params, body);
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
                code.push(Opcode::Return);
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
                let block_value = self.compile_statement_list(statements, code, keep_value);
                if keep_value && !block_value {
                    code.push(Opcode::LoadUndefined);
                }
                code.push(Opcode::ExitScope);
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
                self.compile_stmt(body, code, false);
                code.push(Opcode::Jump(loop_start));

                let loop_end = code.len();
                code[jump_to_end_pos] = Opcode::JumpIfFalse(loop_end);

                if keep_value {
                    code.push(Opcode::LoadUndefined);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn compile_function(
        &mut self,
        name: &Identifier,
        params: &[Identifier],
        body: &[Stmt],
    ) -> usize {
        let mut code = Vec::new();
        self.compile_statement_list(body, &mut code, false);
        code.push(Opcode::LoadUndefined);
        code.push(Opcode::Return);

        let function_id = self.functions.len();
        self.functions.push(CompiledFunction {
            name: name.0.clone(),
            params: params.iter().map(|param| param.0.clone()).collect(),
            code,
        });
        function_id
    }

    fn compile_expr(&mut self, expr: &Expr, code: &mut Vec<Opcode>) {
        match expr {
            Expr::Number(value) => code.push(Opcode::LoadNumber(*value)),
            Expr::Unary { op, expr } => {
                self.compile_expr(expr, code);
                let opcode = match op {
                    UnaryOp::Plus => return,
                    UnaryOp::Minus => Opcode::Neg,
                    UnaryOp::Not => Opcode::Not,
                };
                code.push(opcode);
            }
            Expr::Assign {
                target: Identifier(name),
                value,
            } => {
                self.compile_expr(value, code);
                code.push(Opcode::StoreVariable(name.clone()));
            }
            Expr::Identifier(Identifier(name)) => code.push(Opcode::LoadIdentifier(name.clone())),
            Expr::Call { callee, arguments } => {
                self.compile_expr(callee, code);
                for argument in arguments {
                    self.compile_expr(argument, code);
                }
                code.push(Opcode::Call(arguments.len()));
            }
            Expr::Binary { op, left, right } => {
                self.compile_expr(left, code);
                self.compile_expr(right, code);
                let opcode = match op {
                    BinaryOp::Add => Opcode::Add,
                    BinaryOp::Sub => Opcode::Sub,
                    BinaryOp::Mul => Opcode::Mul,
                    BinaryOp::Div => Opcode::Div,
                    BinaryOp::Equal => Opcode::Eq,
                    BinaryOp::NotEqual => Opcode::Ne,
                    BinaryOp::Less => Opcode::Lt,
                    BinaryOp::LessEqual => Opcode::Le,
                    BinaryOp::Greater => Opcode::Gt,
                    BinaryOp::GreaterEqual => Opcode::Ge,
                };
                code.push(opcode);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chunk, CompiledFunction, Opcode, compile_expression, compile_script};
    use ast::{
        BinaryOp, BindingKind, Expr, FunctionDeclaration, Identifier, Script, Stmt, UnaryOp,
        VariableDeclaration,
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
}
