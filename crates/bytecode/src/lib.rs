#![forbid(unsafe_code)]

use ast::{BinaryOp, BindingKind, Expr, Identifier, Script, Stmt, VariableDeclaration};

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    LoadNumber(f64),
    LoadUndefined,
    LoadIdentifier(String),
    DefineVariable { name: String, mutable: bool },
    StoreVariable(String),
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
    let len = script.statements.len();
    for (index, statement) in script.statements.iter().enumerate() {
        let is_last = index + 1 == len;
        compile_stmt(statement, &mut code, is_last);
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

fn compile_stmt(stmt: &Stmt, code: &mut Vec<Opcode>, is_last: bool) {
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
        }
        Stmt::Expression(expr) => {
            compile_expr(expr, code);
            if !is_last {
                code.push(Opcode::Pop);
            }
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
}
