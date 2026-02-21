#![forbid(unsafe_code)]

use bytecode::{Chunk, Opcode};
use runtime::JsValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    EmptyStack,
    StackUnderflow,
    UnknownIdentifier(String),
    TypeError(&'static str),
}

#[derive(Debug, Default)]
pub struct Vm {
    stack: Vec<JsValue>,
}

impl Vm {
    pub fn execute(&mut self, chunk: &Chunk) -> Result<JsValue, VmError> {
        self.stack.clear();
        for instruction in &chunk.code {
            match instruction {
                Opcode::LoadNumber(value) => self.stack.push(JsValue::Number(*value)),
                Opcode::LoadIdentifier(name) => {
                    return Err(VmError::UnknownIdentifier(name.clone()));
                }
                Opcode::Add => {
                    let result = self.eval_numeric_binary(|lhs, rhs| lhs + rhs)?;
                    self.stack.push(JsValue::Number(result));
                }
                Opcode::Sub => {
                    let result = self.eval_numeric_binary(|lhs, rhs| lhs - rhs)?;
                    self.stack.push(JsValue::Number(result));
                }
                Opcode::Mul => {
                    let result = self.eval_numeric_binary(|lhs, rhs| lhs * rhs)?;
                    self.stack.push(JsValue::Number(result));
                }
                Opcode::Div => {
                    let result = self.eval_numeric_binary(|lhs, rhs| lhs / rhs)?;
                    self.stack.push(JsValue::Number(result));
                }
                Opcode::Halt => break,
            }
        }
        self.stack.pop().ok_or(VmError::EmptyStack)
    }

    fn eval_numeric_binary(&mut self, op: impl FnOnce(f64, f64) -> f64) -> Result<f64, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        match (left, right) {
            (JsValue::Number(lhs), JsValue::Number(rhs)) => Ok(op(lhs, rhs)),
            _ => Err(VmError::TypeError("arithmetic expects numeric operands")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Vm, VmError};
    use bytecode::{Chunk, Opcode};
    use runtime::JsValue;

    #[test]
    fn executes_addition() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::LoadNumber(2.0),
                Opcode::Add,
                Opcode::Halt,
            ],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn executes_mixed_arithmetic() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(20.0),
                Opcode::LoadNumber(5.0),
                Opcode::Div,
                Opcode::LoadNumber(2.0),
                Opcode::Mul,
                Opcode::LoadNumber(3.0),
                Opcode::Sub,
                Opcode::Halt,
            ],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(5.0)));
    }

    #[test]
    fn errors_on_stack_underflow() {
        let chunk = Chunk {
            code: vec![Opcode::Mul, Opcode::Halt],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::StackUnderflow));
    }
}
