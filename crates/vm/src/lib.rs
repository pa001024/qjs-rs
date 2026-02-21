#![forbid(unsafe_code)]

use bytecode::{Chunk, CompiledFunction, Opcode};
use runtime::{JsValue, Realm};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

type BindingId = u64;
type Scope = BTreeMap<String, BindingId>;
type ScopeRef = Rc<RefCell<Scope>>;

#[derive(Debug, Clone, PartialEq)]
struct Binding {
    value: JsValue,
    mutable: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct Closure {
    function_id: usize,
    captured_scopes: Vec<ScopeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    EmptyStack,
    StackUnderflow,
    ScopeUnderflow,
    UnknownIdentifier(String),
    ImmutableBinding(String),
    VariableAlreadyDefined(String),
    UnknownClosure(u64),
    UnknownFunction(usize),
    NotCallable,
    TopLevelReturn,
    InvalidJump(usize),
    TypeError(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionSignal {
    Halt,
    Return,
}

#[derive(Debug, Default)]
pub struct Vm {
    stack: Vec<JsValue>,
    scopes: Vec<ScopeRef>,
    bindings: BTreeMap<BindingId, Binding>,
    next_binding_id: BindingId,
    closures: BTreeMap<u64, Closure>,
    next_closure_id: u64,
}

impl Vm {
    pub fn execute(&mut self, chunk: &Chunk) -> Result<JsValue, VmError> {
        let empty_realm = Realm::default();
        self.execute_in_realm(chunk, &empty_realm)
    }

    pub fn execute_in_realm(&mut self, chunk: &Chunk, realm: &Realm) -> Result<JsValue, VmError> {
        self.stack.clear();
        self.scopes.clear();
        self.scopes.push(Rc::new(RefCell::new(BTreeMap::new())));
        self.bindings.clear();
        self.next_binding_id = 0;
        self.closures.clear();
        self.next_closure_id = 0;

        match self.execute_code(&chunk.code, &chunk.functions, realm, false)? {
            ExecutionSignal::Halt => self.stack.pop().ok_or(VmError::EmptyStack),
            ExecutionSignal::Return => Err(VmError::TopLevelReturn),
        }
    }

    fn execute_code(
        &mut self,
        code: &[Opcode],
        functions: &[CompiledFunction],
        realm: &Realm,
        allow_return: bool,
    ) -> Result<ExecutionSignal, VmError> {
        let mut pc = 0usize;
        while pc < code.len() {
            match &code[pc] {
                Opcode::LoadNumber(value) => self.stack.push(JsValue::Number(*value)),
                Opcode::LoadUndefined => self.stack.push(JsValue::Undefined),
                Opcode::LoadIdentifier(name) => {
                    let value = if let Some(binding_id) = self.resolve_binding_id(name) {
                        let binding = self
                            .bindings
                            .get(&binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
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
                    {
                        let scope_ref = self.current_scope_ref()?;
                        if scope_ref.borrow().contains_key(name) {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                    }
                    let binding_id = self.create_binding(value, *mutable);
                    let scope_ref = self.current_scope_ref()?;
                    if scope_ref
                        .borrow_mut()
                        .insert(name.clone(), binding_id)
                        .is_some()
                    {
                        return Err(VmError::VariableAlreadyDefined(name.clone()));
                    }
                }
                Opcode::DefineFunction { name, function_id } => {
                    if *function_id >= functions.len() {
                        return Err(VmError::UnknownFunction(*function_id));
                    }
                    let closure_id = self.next_closure_id;
                    self.next_closure_id += 1;

                    {
                        let scope_ref = self.current_scope_ref()?;
                        if scope_ref.borrow().contains_key(name) {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                    }
                    let function_binding =
                        self.create_binding(JsValue::Function(closure_id), false);
                    let scope_ref = self.current_scope_ref()?;
                    if scope_ref
                        .borrow_mut()
                        .insert(name.clone(), function_binding)
                        .is_some()
                    {
                        return Err(VmError::VariableAlreadyDefined(name.clone()));
                    }

                    let captured_scopes = self.scopes.clone();
                    self.closures.insert(
                        closure_id,
                        Closure {
                            function_id: *function_id,
                            captured_scopes,
                        },
                    );
                }
                Opcode::StoreVariable(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let binding_id = self
                        .resolve_binding_id(name)
                        .ok_or_else(|| VmError::UnknownIdentifier(name.clone()))?;
                    let binding = self
                        .bindings
                        .get_mut(&binding_id)
                        .ok_or(VmError::ScopeUnderflow)?;
                    if !binding.mutable {
                        return Err(VmError::ImmutableBinding(name.clone()));
                    }
                    binding.value = value.clone();
                    self.stack.push(value);
                }
                Opcode::EnterScope => self.scopes.push(Rc::new(RefCell::new(BTreeMap::new()))),
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
                Opcode::Neg => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match value {
                        JsValue::Number(number) => self.stack.push(JsValue::Number(-number)),
                        _ => return Err(VmError::TypeError("unary '-' expects numeric operand")),
                    }
                }
                Opcode::Not => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(JsValue::Bool(!self.is_truthy(&value)));
                }
                Opcode::Eq => {
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(JsValue::Bool(left == right));
                }
                Opcode::Ne => {
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(JsValue::Bool(left != right));
                }
                Opcode::Lt => {
                    let result = self.eval_numeric_compare(|lhs, rhs| lhs < rhs)?;
                    self.stack.push(JsValue::Bool(result));
                }
                Opcode::Le => {
                    let result = self.eval_numeric_compare(|lhs, rhs| lhs <= rhs)?;
                    self.stack.push(JsValue::Bool(result));
                }
                Opcode::Gt => {
                    let result = self.eval_numeric_compare(|lhs, rhs| lhs > rhs)?;
                    self.stack.push(JsValue::Bool(result));
                }
                Opcode::Ge => {
                    let result = self.eval_numeric_compare(|lhs, rhs| lhs >= rhs)?;
                    self.stack.push(JsValue::Bool(result));
                }
                Opcode::JumpIfFalse(target) => {
                    let condition = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    if !self.is_truthy(&condition) {
                        if *target >= code.len() {
                            return Err(VmError::InvalidJump(*target));
                        }
                        pc = *target;
                        continue;
                    }
                }
                Opcode::Jump(target) => {
                    if *target >= code.len() {
                        return Err(VmError::InvalidJump(*target));
                    }
                    pc = *target;
                    continue;
                }
                Opcode::Call(arg_count) => {
                    let result = self.execute_call(*arg_count, functions, realm)?;
                    self.stack.push(result);
                }
                Opcode::Return => {
                    if !allow_return {
                        return Err(VmError::TopLevelReturn);
                    }
                    return Ok(ExecutionSignal::Return);
                }
                Opcode::Pop => {
                    self.stack.pop().ok_or(VmError::StackUnderflow)?;
                }
                Opcode::Halt => return Ok(ExecutionSignal::Halt),
            }

            pc += 1;
        }

        Ok(ExecutionSignal::Halt)
    }

    fn execute_call(
        &mut self,
        arg_count: usize,
        functions: &[CompiledFunction],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
            args.push(value);
        }
        args.reverse();

        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let closure_id = match callee {
            JsValue::Function(id) => id,
            _ => return Err(VmError::NotCallable),
        };

        let closure = self
            .closures
            .get(&closure_id)
            .cloned()
            .ok_or(VmError::UnknownClosure(closure_id))?;
        let function = functions
            .get(closure.function_id)
            .cloned()
            .ok_or(VmError::UnknownFunction(closure.function_id))?;

        let mut frame_scope: Scope = BTreeMap::new();
        for (index, param_name) in function.params.iter().enumerate() {
            let value = args.get(index).cloned().unwrap_or(JsValue::Undefined);
            let binding_id = self.create_binding(value, true);
            frame_scope.insert(param_name.clone(), binding_id);
        }

        let saved_stack = std::mem::take(&mut self.stack);
        let saved_scopes = std::mem::take(&mut self.scopes);

        self.scopes = closure.captured_scopes;
        self.scopes.push(Rc::new(RefCell::new(frame_scope)));
        self.stack = Vec::new();

        let signal = self.execute_code(&function.code, functions, realm, true);
        let value = match signal {
            Ok(ExecutionSignal::Return) => self.stack.pop().unwrap_or(JsValue::Undefined),
            Ok(ExecutionSignal::Halt) => JsValue::Undefined,
            Err(err) => {
                self.stack = saved_stack;
                self.scopes = saved_scopes;
                return Err(err);
            }
        };

        self.stack = saved_stack;
        self.scopes = saved_scopes;
        Ok(value)
    }

    fn create_binding(&mut self, value: JsValue, mutable: bool) -> BindingId {
        let id = self.next_binding_id;
        self.next_binding_id += 1;
        self.bindings.insert(id, Binding { value, mutable });
        id
    }

    fn current_scope_ref(&self) -> Result<ScopeRef, VmError> {
        self.scopes.last().cloned().ok_or(VmError::ScopeUnderflow)
    }

    fn resolve_binding_id(&self, name: &str) -> Option<BindingId> {
        for scope_ref in self.scopes.iter().rev() {
            if let Some(binding_id) = scope_ref.borrow().get(name).copied() {
                return Some(binding_id);
            }
        }
        None
    }

    fn eval_numeric_binary(&mut self, op: impl FnOnce(f64, f64) -> f64) -> Result<f64, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        match (left, right) {
            (JsValue::Number(lhs), JsValue::Number(rhs)) => Ok(op(lhs, rhs)),
            _ => Err(VmError::TypeError("arithmetic expects numeric operands")),
        }
    }

    fn eval_numeric_compare(&mut self, op: impl FnOnce(f64, f64) -> bool) -> Result<bool, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        match (left, right) {
            (JsValue::Number(lhs), JsValue::Number(rhs)) => Ok(op(lhs, rhs)),
            _ => Err(VmError::TypeError("comparison expects numeric operands")),
        }
    }

    fn is_truthy(&self, value: &JsValue) -> bool {
        match value {
            JsValue::Undefined => false,
            JsValue::Bool(boolean) => *boolean,
            JsValue::Number(number) => *number != 0.0 && !number.is_nan(),
            JsValue::Function(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Vm, VmError};
    use bytecode::{Chunk, CompiledFunction, Opcode};
    use runtime::{JsValue, Realm};

    fn empty_chunk(code: Vec<Opcode>) -> Chunk {
        Chunk {
            code,
            functions: vec![],
        }
    }

    #[test]
    fn executes_addition() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::LoadNumber(2.0),
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn executes_mixed_arithmetic() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(20.0),
            Opcode::LoadNumber(5.0),
            Opcode::Div,
            Opcode::LoadNumber(2.0),
            Opcode::Mul,
            Opcode::LoadNumber(3.0),
            Opcode::Sub,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(5.0)));
    }

    #[test]
    fn errors_on_stack_underflow() {
        let chunk = empty_chunk(vec![Opcode::Mul, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::StackUnderflow));
    }

    #[test]
    fn resolves_identifier_from_realm() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::LoadNumber(2.0),
            Opcode::Mul,
            Opcode::Halt,
        ]);
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
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadNumber(2.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn errors_when_assigning_to_const() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: false,
            },
            Opcode::LoadNumber(2.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::ImmutableBinding("x".to_string()))
        );
    }

    #[test]
    fn supports_scope_shadowing() {
        let chunk = empty_chunk(vec![
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
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn assignment_updates_nearest_scope_binding() {
        let chunk = empty_chunk(vec![
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
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn executes_conditional_jumps() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(0.0),
            Opcode::JumpIfFalse(4),
            Opcode::LoadNumber(1.0),
            Opcode::Jump(5),
            Opcode::LoadNumber(2.0),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn executes_loop_with_jumps() {
        let chunk = empty_chunk(vec![
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
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn errors_on_invalid_jump_target() {
        let chunk = empty_chunk(vec![Opcode::Jump(99), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::InvalidJump(99)));
    }

    #[test]
    fn executes_function_call() {
        let chunk = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "add".to_string(),
                    function_id: 0,
                },
                Opcode::LoadIdentifier("add".to_string()),
                Opcode::LoadNumber(20.0),
                Opcode::LoadNumber(22.0),
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
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn function_call_observes_outer_assignment() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(10.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::DefineFunction {
                    name: "add".to_string(),
                    function_id: 0,
                },
                Opcode::LoadNumber(20.0),
                Opcode::StoreVariable("x".to_string()),
                Opcode::Pop,
                Opcode::LoadIdentifier("add".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Call(1),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "add".to_string(),
                params: vec!["v".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("x".to_string()),
                    Opcode::LoadIdentifier("v".to_string()),
                    Opcode::Add,
                    Opcode::Return,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(21.0)));
    }

    #[test]
    fn reports_not_callable() {
        let chunk = empty_chunk(vec![Opcode::LoadNumber(1.0), Opcode::Call(0), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::NotCallable));
    }

    #[test]
    fn executes_unary_and_comparison_ops() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(2.0),
            Opcode::Neg,
            Opcode::LoadNumber(-2.0),
            Opcode::Eq,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn executes_logical_not_with_truthiness() {
        let chunk = empty_chunk(vec![Opcode::LoadNumber(0.0), Opcode::Not, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn executes_relational_comparison() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(3.0),
            Opcode::LoadNumber(2.0),
            Opcode::Gt,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }
}
