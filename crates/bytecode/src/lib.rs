#![forbid(unsafe_code)]

use ast::{BinaryOp, Expr, Identifier};

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    LoadNumber(f64),
    LoadIdentifier(String),
    Add,
    Sub,
    Mul,
    Div,
    Halt,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chunk {
    pub code: Vec<Opcode>,
}

pub fn compile_expression(expr: &Expr) -> Chunk {
    let mut code = Vec::new();
    compile_expr(expr, &mut code);
    code.push(Opcode::Halt);
    Chunk { code }
}

fn compile_expr(expr: &Expr, code: &mut Vec<Opcode>) {
    match expr {
        Expr::Number(value) => code.push(Opcode::LoadNumber(*value)),
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
    use super::{Chunk, Opcode, compile_expression};
    use ast::{BinaryOp, Expr};

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
}
