#![forbid(unsafe_code)]

use ast::{BinaryOp, Expr, Identifier};

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    LoadNumber(f64),
    LoadIdentifier(String),
    Add,
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
        Expr::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } => {
            compile_expr(left, code);
            compile_expr(right, code);
            code.push(Opcode::Add);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Chunk, Opcode, compile_expression};
    use ast::{BinaryOp, Expr};

    #[test]
    fn compiles_binary_add() {
        let expr = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::Number(2.0)),
        };

        let chunk = compile_expression(&expr);
        let expected = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::LoadNumber(2.0),
                Opcode::Add,
                Opcode::Halt,
            ],
        };

        assert_eq!(chunk, expected);
    }
}
