#![forbid(unsafe_code)]

use ast::{BinaryOp, BindingKind, Expr, Identifier, Script, Stmt, VariableDeclaration};

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    LoadNumber(f64),
    LoadUndefined,
    LoadIdentifier(String),
    DefineVariable { name: String, mutable: bool },
    StoreVariable(String),
    EnterScope,
    ExitScope,
    Add,
    Sub,
    Mul,
    Div,
    Pop,
    Halt,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chunk {
    pub code: Vec<Opcode>,
}

pub fn compile_script(script: &Script) -> Chunk {
    let mut code = Vec::new();
    let produced_value = compile_statement_list(&script.statements, &mut code, true);
    if !produced_value {
        code.push(Opcode::LoadUndefined);
    }
    code.push(Opcode::Halt);
    Chunk { code }
}

pub fn compile_expression(expr: &Expr) -> Chunk {
    let mut code = Vec::new();
    compile_expr(expr, &mut code);
    code.push(Opcode::Halt);
    Chunk { code }
}

fn compile_statement_list(
    statements: &[Stmt],
    code: &mut Vec<Opcode>,
    preserve_value: bool,
) -> bool {
    let mut produced_value = false;
    let len = statements.len();

    for (index, stmt) in statements.iter().enumerate() {
        let is_last = index + 1 == len;
        let keep_value = preserve_value && is_last;
        produced_value = compile_stmt(stmt, code, keep_value);
    }

    produced_value
}

fn compile_stmt(stmt: &Stmt, code: &mut Vec<Opcode>, keep_value: bool) -> bool {
    match stmt {
        Stmt::VariableDeclaration(VariableDeclaration {
            kind,
            name: Identifier(binding_name),
            initializer,
        }) => {
            if let Some(expr) = initializer {
                compile_expr(expr, code);
            } else {
                code.push(Opcode::LoadUndefined);
            }
            code.push(Opcode::DefineVariable {
                name: binding_name.clone(),
                mutable: matches!(kind, BindingKind::Let),
            });
            false
        }
        Stmt::Expression(expr) => {
            compile_expr(expr, code);
            if !keep_value {
                code.push(Opcode::Pop);
                false
            } else {
                true
            }
        }
        Stmt::Block(statements) => {
            code.push(Opcode::EnterScope);
            let block_value = compile_statement_list(statements, code, keep_value);
            if keep_value && !block_value {
                code.push(Opcode::LoadUndefined);
            }
            code.push(Opcode::ExitScope);
            keep_value
        }
    }
}

fn compile_expr(expr: &Expr, code: &mut Vec<Opcode>) {
    match expr {
        Expr::Number(value) => code.push(Opcode::LoadNumber(*value)),
        Expr::Assign {
            target: Identifier(name),
            value,
        } => {
            compile_expr(value, code);
            code.push(Opcode::StoreVariable(name.clone()));
        }
        Expr::Identifier(Identifier(name)) => code.push(Opcode::LoadIdentifier(name.clone())),
        Expr::Binary { op, left, right } => {
            compile_expr(left, code);
            compile_expr(right, code);
            let opcode = match op {
                BinaryOp::Add => Opcode::Add,
                BinaryOp::Sub => Opcode::Sub,
                BinaryOp::Mul => Opcode::Mul,
                BinaryOp::Div => Opcode::Div,
            };
            code.push(opcode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chunk, Opcode, compile_expression, compile_script};
    use ast::{BinaryOp, BindingKind, Expr, Identifier, Script, Stmt, VariableDeclaration};

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
        };

        assert_eq!(chunk, expected);
    }
}
