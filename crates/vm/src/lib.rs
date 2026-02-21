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
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (left, right) {
                        (JsValue::Number(lhs), JsValue::Number(rhs)) => {
                            self.stack.push(JsValue::Number(lhs + rhs));
                        }
                        _ => return Err(VmError::TypeError("addition expects numeric operands")),
                    }
                }
                Opcode::Halt => break,
            }
        }
        self.stack.pop().ok_or(VmError::EmptyStack)
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
    fn errors_on_stack_underflow() {
        let chunk = Chunk {
            code: vec![Opcode::Add, Opcode::Halt],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::StackUnderflow));
    }
}
