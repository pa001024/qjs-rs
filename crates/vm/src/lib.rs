#![forbid(unsafe_code)]

use bytecode::{Chunk, Opcode};
use runtime::{JsValue, Realm};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
struct Binding {
    value: JsValue,
    mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    EmptyStack,
    StackUnderflow,
    ScopeUnderflow,
    UnknownIdentifier(String),
    ImmutableBinding(String),
    VariableAlreadyDefined(String),
    TypeError(&'static str),
}

#[derive(Debug, Default)]
pub struct Vm {
    stack: Vec<JsValue>,
    scopes: Vec<BTreeMap<String, Binding>>,
}

impl Vm {
    pub fn execute(&mut self, chunk: &Chunk) -> Result<JsValue, VmError> {
        let empty_realm = Realm::default();
        self.execute_in_realm(chunk, &empty_realm)
    }

    pub fn execute_in_realm(&mut self, chunk: &Chunk, realm: &Realm) -> Result<JsValue, VmError> {
        self.stack.clear();
        self.scopes.clear();
        self.scopes.push(BTreeMap::new());
        for instruction in &chunk.code {
            match instruction {
                Opcode::LoadNumber(value) => self.stack.push(JsValue::Number(*value)),
                Opcode::LoadUndefined => self.stack.push(JsValue::Undefined),
                Opcode::LoadIdentifier(name) => {
                    let value = if let Some(binding) = self.resolve_binding(name) {
                        binding.value.clone()
                    } else {
                        realm
                            .resolve_identifier(name)
                            .ok_or_else(|| VmError::UnknownIdentifier(name.clone()))?
                    };
                    self.stack.push(value);
                }
                Opcode::DefineVariable { name, mutable } => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let scope = self.current_scope_mut()?;
                    if scope.contains_key(name) {
                        return Err(VmError::VariableAlreadyDefined(name.clone()));
                    }
                    scope.insert(
                        name.clone(),
                        Binding {
                            value,
                            mutable: *mutable,
                        },
                    );
                }
                Opcode::StoreVariable(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let binding = self
                        .resolve_binding_mut(name)
                        .ok_or_else(|| VmError::UnknownIdentifier(name.clone()))?;
                    if !binding.mutable {
                        return Err(VmError::ImmutableBinding(name.clone()));
                    }
                    binding.value = value.clone();
                    self.stack.push(value);
                }
                Opcode::EnterScope => self.scopes.push(BTreeMap::new()),
                Opcode::ExitScope => {
                    if self.scopes.pop().is_none() || self.scopes.is_empty() {
                        return Err(VmError::ScopeUnderflow);
                    }
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
                Opcode::Pop => {
                    self.stack.pop().ok_or(VmError::StackUnderflow)?;
                }
                Opcode::Halt => break,
            }
        }
        self.stack.pop().ok_or(VmError::EmptyStack)
    }

    fn current_scope_mut(&mut self) -> Result<&mut BTreeMap<String, Binding>, VmError> {
        self.scopes.last_mut().ok_or(VmError::ScopeUnderflow)
    }

    fn resolve_binding(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn resolve_binding_mut(&mut self, name: &str) -> Option<&mut Binding> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
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
    use runtime::{JsValue, Realm};

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

    #[test]
    fn resolves_identifier_from_realm() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::LoadNumber(2.0),
                Opcode::Mul,
                Opcode::Halt,
            ],
        };
        let mut realm = Realm::default();
        realm.define_global("x", JsValue::Number(21.0));
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute_in_realm(&chunk, &realm),
            Ok(JsValue::Number(42.0))
        );
    }

    #[test]
    fn executes_let_declaration_and_assignment() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::LoadNumber(2.0),
                Opcode::StoreVariable("x".to_string()),
                Opcode::Halt,
            ],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn errors_when_assigning_to_const() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: false,
                },
                Opcode::LoadNumber(2.0),
                Opcode::StoreVariable("x".to_string()),
                Opcode::Halt,
            ],
        };
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::ImmutableBinding("x".to_string()))
        );
    }

    #[test]
    fn supports_scope_shadowing() {
        let chunk = Chunk {
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
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn assignment_updates_nearest_scope_binding() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::EnterScope,
                Opcode::LoadNumber(2.0),
                Opcode::StoreVariable("x".to_string()),
                Opcode::Pop,
                Opcode::ExitScope,
                Opcode::LoadIdentifier("x".to_string()),
                Opcode::Halt,
            ],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }
}
