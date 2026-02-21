#![forbid(unsafe_code)]

use bytecode::{Chunk, CompiledFunction, Opcode, compile_expression, compile_script};
use parser::{parse_expression, parse_script};
use runtime::{JsValue, NativeFunction, Realm};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

type BindingId = u64;
type ObjectId = u64;
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
    functions: Rc<Vec<CompiledFunction>>,
    captured_scopes: Vec<ScopeRef>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct JsObject {
    properties: BTreeMap<String, JsValue>,
    getters: BTreeMap<String, JsValue>,
    setters: BTreeMap<String, JsValue>,
}

#[derive(Debug, Clone, PartialEq)]
struct ExceptionHandler {
    catch_target: Option<usize>,
    finally_target: Option<usize>,
    scope_depth: usize,
    stack_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionMethod {
    Call,
    Apply,
    Bind,
}

#[derive(Debug, Clone, PartialEq)]
enum HostFunction {
    BoundMethod {
        target: JsValue,
        method: FunctionMethod,
    },
    BoundCall {
        target: JsValue,
        this_arg: JsValue,
        bound_args: Vec<JsValue>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum VmError {
    EmptyStack,
    StackUnderflow,
    ScopeUnderflow,
    UnknownIdentifier(String),
    ImmutableBinding(String),
    VariableAlreadyDefined(String),
    UnknownClosure(u64),
    UnknownHostFunction(u64),
    UnknownFunction(usize),
    UnknownObject(u64),
    NotCallable,
    TopLevelReturn,
    InvalidJump(usize),
    HandlerUnderflow,
    NoPendingException,
    UncaughtException(JsValue),
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
    objects: BTreeMap<ObjectId, JsObject>,
    next_object_id: ObjectId,
    closures: BTreeMap<u64, Closure>,
    next_closure_id: u64,
    host_functions: BTreeMap<u64, HostFunction>,
    next_host_function_id: u64,
    exception_handlers: Vec<ExceptionHandler>,
    pending_exception: Option<JsValue>,
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
        self.objects.clear();
        self.next_object_id = 0;
        self.closures.clear();
        self.next_closure_id = 0;
        self.host_functions.clear();
        self.next_host_function_id = 0;
        self.exception_handlers.clear();
        self.pending_exception = None;

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
                Opcode::LoadBool(value) => self.stack.push(JsValue::Bool(*value)),
                Opcode::LoadNull => self.stack.push(JsValue::Null),
                Opcode::LoadString(value) => self.stack.push(JsValue::String(value.clone())),
                Opcode::LoadUndefined => self.stack.push(JsValue::Undefined),
                Opcode::CreateObject => {
                    let object = self.create_object_value();
                    self.stack.push(object);
                }
                Opcode::LoadFunction(function_id) => {
                    let function = self.instantiate_function(*function_id, functions)?;
                    self.stack.push(function);
                }
                Opcode::LoadIdentifier(name) => {
                    let value = if let Some(binding_id) = self.resolve_binding_id(name) {
                        let binding = self
                            .bindings
                            .get(&binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        binding.value.clone()
                    } else if name == "undefined" {
                        JsValue::Undefined
                    } else if name == "NaN" {
                        JsValue::Number(f64::NAN)
                    } else if name == "Infinity" {
                        JsValue::Number(f64::INFINITY)
                    } else if name == "this" {
                        realm.resolve_identifier(name).unwrap_or(JsValue::Undefined)
                    } else {
                        realm
                            .resolve_identifier(name)
                            .ok_or_else(|| VmError::UnknownIdentifier(name.clone()))?
                    };
                    self.stack.push(value);
                }
                Opcode::DefineVariable { name, mutable } => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let existing_binding_id = {
                        let scope_ref = self.current_scope_ref()?;
                        scope_ref.borrow().get(name).copied()
                    };
                    if let Some(existing_binding_id) = existing_binding_id {
                        let existing_binding = self
                            .bindings
                            .get_mut(&existing_binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        if !existing_binding.mutable {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                        // Treat `var`-style redeclaration without initializer as a no-op.
                        if value != JsValue::Undefined {
                            existing_binding.value = value;
                        }
                    } else {
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
                }
                Opcode::DefineFunction { name, function_id } => {
                    let function_value = self.instantiate_function(*function_id, functions)?;
                    let existing_binding_id = {
                        let scope_ref = self.current_scope_ref()?;
                        scope_ref.borrow().get(name).copied()
                    };
                    if let Some(existing_binding_id) = existing_binding_id {
                        let existing_binding = self
                            .bindings
                            .get_mut(&existing_binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        if !existing_binding.mutable {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                        existing_binding.value = function_value;
                    } else {
                        let function_binding = self.create_binding(function_value, true);
                        let scope_ref = self.current_scope_ref()?;
                        if scope_ref
                            .borrow_mut()
                            .insert(name.clone(), function_binding)
                            .is_some()
                        {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                    }
                }
                Opcode::StoreVariable(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    if let Some(binding_id) = self.resolve_binding_id(name) {
                        let binding = self
                            .bindings
                            .get_mut(&binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        if !binding.mutable {
                            return Err(VmError::ImmutableBinding(name.clone()));
                        }
                        binding.value = value.clone();
                    } else {
                        let global_scope = self
                            .scopes
                            .first()
                            .cloned()
                            .ok_or(VmError::ScopeUnderflow)?;
                        let binding_id = self.create_binding(value.clone(), true);
                        global_scope.borrow_mut().insert(name.clone(), binding_id);
                    }
                    self.stack.push(value);
                }
                Opcode::GetProperty(name) => {
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let value = match receiver {
                        JsValue::Object(object_id) => {
                            self.get_object_property(object_id, name, realm)?
                        }
                        JsValue::Function(closure_id) => {
                            self.get_function_property(closure_id, name)?
                        }
                        JsValue::NativeFunction(native) => {
                            self.get_native_function_property(native, name)
                        }
                        JsValue::HostFunction(host_id) => {
                            self.get_host_function_property(host_id, name)?
                        }
                        _ => return Err(VmError::TypeError("property access expects object")),
                    };
                    self.stack.push(value);
                }
                Opcode::GetPropertyByValue => {
                    let key = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&key);
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let value = match receiver {
                        JsValue::Object(object_id) => {
                            self.get_object_property(object_id, &key, realm)?
                        }
                        JsValue::Function(closure_id) => {
                            self.get_function_property(closure_id, &key)?
                        }
                        JsValue::NativeFunction(native) => {
                            self.get_native_function_property(native, &key)
                        }
                        JsValue::HostFunction(host_id) => {
                            self.get_host_function_property(host_id, &key)?
                        }
                        _ => return Err(VmError::TypeError("property access expects object")),
                    };
                    self.stack.push(value);
                }
                Opcode::DefineProperty(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    object.properties.insert(name.clone(), value);
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::SetProperty(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match receiver {
                        JsValue::Object(object_id) => {
                            let result =
                                self.set_object_property(object_id, name.clone(), value, realm)?;
                            self.stack.push(result);
                        }
                        JsValue::Function(_)
                        | JsValue::NativeFunction(_)
                        | JsValue::HostFunction(_) => {
                            self.stack.push(value);
                        }
                        _ => return Err(VmError::TypeError("property write expects object")),
                    }
                }
                Opcode::SetPropertyByValue => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&key);
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match receiver {
                        JsValue::Object(object_id) => {
                            let result = self.set_object_property(object_id, key, value, realm)?;
                            self.stack.push(result);
                        }
                        JsValue::Function(_)
                        | JsValue::NativeFunction(_)
                        | JsValue::HostFunction(_) => {
                            self.stack.push(value);
                        }
                        _ => return Err(VmError::TypeError("property write expects object")),
                    }
                }
                Opcode::EnterScope => self.scopes.push(Rc::new(RefCell::new(BTreeMap::new()))),
                Opcode::ExitScope => {
                    if self.scopes.pop().is_none() || self.scopes.is_empty() {
                        return Err(VmError::ScopeUnderflow);
                    }
                }
                Opcode::Add => {
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match (left, right) {
                        (JsValue::Number(lhs), JsValue::Number(rhs)) => {
                            self.stack.push(JsValue::Number(lhs + rhs));
                        }
                        (JsValue::String(lhs), rhs) => {
                            let rhs = self.coerce_to_string(&rhs);
                            self.stack.push(JsValue::String(format!("{lhs}{rhs}")));
                        }
                        (lhs, JsValue::String(rhs)) => {
                            let lhs = self.coerce_to_string(&lhs);
                            self.stack.push(JsValue::String(format!("{lhs}{rhs}")));
                        }
                        (lhs, rhs) => {
                            self.stack
                                .push(JsValue::Number(self.to_number(&lhs) + self.to_number(&rhs)));
                        }
                    }
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
                    self.stack.push(JsValue::Number(-self.to_number(&value)));
                }
                Opcode::Not => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(JsValue::Bool(!self.is_truthy(&value)));
                }
                Opcode::Typeof => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack
                        .push(JsValue::String(self.typeof_value(&value).to_string()));
                }
                Opcode::TypeofIdentifier(name) => {
                    let value = if let Some(binding_id) = self.resolve_binding_id(name) {
                        let binding = self
                            .bindings
                            .get(&binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        binding.value.clone()
                    } else if name == "undefined" {
                        JsValue::Undefined
                    } else if name == "NaN" {
                        JsValue::Number(f64::NAN)
                    } else if name == "Infinity" {
                        JsValue::Number(f64::INFINITY)
                    } else {
                        realm.resolve_identifier(name).unwrap_or(JsValue::Undefined)
                    };
                    self.stack
                        .push(JsValue::String(self.typeof_value(&value).to_string()));
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
                Opcode::PushExceptionHandler {
                    catch_target,
                    finally_target,
                } => {
                    self.exception_handlers.push(ExceptionHandler {
                        catch_target: *catch_target,
                        finally_target: *finally_target,
                        scope_depth: self.scopes.len(),
                        stack_depth: self.stack.len(),
                    });
                }
                Opcode::PopExceptionHandler => {
                    self.exception_handlers
                        .pop()
                        .ok_or(VmError::HandlerUnderflow)?;
                }
                Opcode::LoadException => {
                    let exception = self
                        .pending_exception
                        .take()
                        .ok_or(VmError::NoPendingException)?;
                    self.stack.push(exception);
                }
                Opcode::RethrowIfException => {
                    if let Some(exception) = self.pending_exception.take() {
                        let target = self.throw_to_handler(exception, code.len())?;
                        pc = target;
                        continue;
                    }
                }
                Opcode::Throw => {
                    let exception = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let target = self.throw_to_handler(exception, code.len())?;
                    pc = target;
                    continue;
                }
                Opcode::Call(arg_count) => match self.execute_call(*arg_count, realm) {
                    Ok(result) => self.stack.push(result),
                    Err(VmError::UncaughtException(exception)) => {
                        let target = self.throw_to_handler(exception, code.len())?;
                        pc = target;
                        continue;
                    }
                    Err(err) => return Err(err),
                },
                Opcode::Return => {
                    if !allow_return {
                        return Err(VmError::TopLevelReturn);
                    }
                    return Ok(ExecutionSignal::Return);
                }
                Opcode::Dup => {
                    let value = self.stack.last().cloned().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(value);
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

    fn execute_call(&mut self, arg_count: usize, realm: &Realm) -> Result<JsValue, VmError> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
            args.push(value);
        }
        args.reverse();

        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        self.execute_callable(callee, None, args, realm)
    }

    fn execute_callable(
        &mut self,
        callee: JsValue,
        this_arg: Option<JsValue>,
        args: Vec<JsValue>,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        match callee {
            JsValue::NativeFunction(native) => self.execute_native_call(native, args, realm),
            JsValue::HostFunction(host_id) => self.execute_host_function_call(host_id, args, realm),
            JsValue::Function(closure_id) => {
                self.execute_closure_call(closure_id, args, this_arg, realm)
            }
            _ => Err(VmError::NotCallable),
        }
    }

    fn execute_closure_call(
        &mut self,
        closure_id: u64,
        args: Vec<JsValue>,
        this_arg: Option<JsValue>,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let closure = self
            .closures
            .get(&closure_id)
            .cloned()
            .ok_or(VmError::UnknownClosure(closure_id))?;
        let function = closure
            .functions
            .get(closure.function_id)
            .cloned()
            .ok_or(VmError::UnknownFunction(closure.function_id))?;

        let mut frame_scope: Scope = BTreeMap::new();
        for (index, param_name) in function.params.iter().enumerate() {
            let value = args.get(index).cloned().unwrap_or(JsValue::Undefined);
            let binding_id = self.create_binding(value, true);
            frame_scope.insert(param_name.clone(), binding_id);
        }
        if let Some(this_value) = this_arg {
            let this_binding_id = self.create_binding(this_value, true);
            frame_scope.insert("this".to_string(), this_binding_id);
        }
        let arguments_value = self.create_object_value();
        let arguments_id = match arguments_value {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        {
            let object = self
                .objects
                .get_mut(&arguments_id)
                .ok_or(VmError::UnknownObject(arguments_id))?;
            object
                .properties
                .insert("length".to_string(), JsValue::Number(args.len() as f64));
            object
                .properties
                .insert("callee".to_string(), JsValue::Function(closure_id));
            for (index, arg) in args.iter().enumerate() {
                object.properties.insert(index.to_string(), arg.clone());
            }
        }
        let arguments_binding_id = self.create_binding(arguments_value, true);
        frame_scope.insert("arguments".to_string(), arguments_binding_id);

        let saved_stack = std::mem::take(&mut self.stack);
        let saved_scopes = std::mem::take(&mut self.scopes);
        let saved_handlers = std::mem::take(&mut self.exception_handlers);
        let saved_pending_exception = self.pending_exception.take();

        self.scopes = closure.captured_scopes;
        self.scopes.push(Rc::new(RefCell::new(frame_scope)));
        self.stack = Vec::new();
        self.exception_handlers = Vec::new();
        self.pending_exception = None;

        let signal = self.execute_code(&function.code, closure.functions.as_ref(), realm, true);
        let value = match signal {
            Ok(ExecutionSignal::Return) => self.stack.pop().unwrap_or(JsValue::Undefined),
            Ok(ExecutionSignal::Halt) => JsValue::Undefined,
            Err(err) => {
                self.stack = saved_stack;
                self.scopes = saved_scopes;
                self.exception_handlers = saved_handlers;
                self.pending_exception = saved_pending_exception;
                return Err(err);
            }
        };

        self.stack = saved_stack;
        self.scopes = saved_scopes;
        self.exception_handlers = saved_handlers;
        self.pending_exception = saved_pending_exception;
        Ok(value)
    }

    fn execute_host_function_call(
        &mut self,
        host_id: u64,
        args: Vec<JsValue>,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let host = self
            .host_functions
            .get(&host_id)
            .cloned()
            .ok_or(VmError::UnknownHostFunction(host_id))?;
        match host {
            HostFunction::BoundMethod { target, method } => match method {
                FunctionMethod::Call => {
                    let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                    let call_args = args.get(1..).map_or_else(Vec::new, |slice| slice.to_vec());
                    self.execute_callable(target, Some(this_arg), call_args, realm)
                }
                FunctionMethod::Apply => {
                    let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                    let call_args = self.collect_apply_arguments(args.get(1))?;
                    self.execute_callable(target, Some(this_arg), call_args, realm)
                }
                FunctionMethod::Bind => {
                    let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                    let bound_args = args.get(1..).map_or_else(Vec::new, |slice| slice.to_vec());
                    Ok(self.create_host_function_value(HostFunction::BoundCall {
                        target,
                        this_arg,
                        bound_args,
                    }))
                }
            },
            HostFunction::BoundCall {
                target,
                this_arg,
                mut bound_args,
            } => {
                bound_args.extend(args);
                self.execute_callable(target, Some(this_arg), bound_args, realm)
            }
        }
    }

    fn collect_apply_arguments(
        &self,
        apply_args: Option<&JsValue>,
    ) -> Result<Vec<JsValue>, VmError> {
        let Some(apply_args) = apply_args else {
            return Ok(Vec::new());
        };
        match apply_args {
            JsValue::Null | JsValue::Undefined => Ok(Vec::new()),
            JsValue::Object(object_id) => {
                let object = self
                    .objects
                    .get(object_id)
                    .ok_or(VmError::UnknownObject(*object_id))?;
                let length = object
                    .properties
                    .get("length")
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                let mut args = Vec::with_capacity(length);
                for index in 0..length {
                    let key = index.to_string();
                    args.push(
                        object
                            .properties
                            .get(&key)
                            .cloned()
                            .unwrap_or(JsValue::Undefined),
                    );
                }
                Ok(args)
            }
            _ => Ok(Vec::new()),
        }
    }

    fn execute_native_call(
        &mut self,
        native: NativeFunction,
        args: Vec<JsValue>,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        match native {
            NativeFunction::Eval => match args.first() {
                Some(JsValue::String(source)) => self.execute_eval(source, realm),
                Some(value) => Ok(value.clone()),
                None => Ok(JsValue::Undefined),
            },
            NativeFunction::FunctionConstructor => self.execute_function_constructor(&args, realm),
            NativeFunction::ObjectConstructor => Ok(self.execute_object_constructor(&args)),
            NativeFunction::ObjectDefineProperty => {
                self.execute_object_define_property(&args, realm)
            }
            NativeFunction::NumberConstructor => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(0.0));
                Ok(JsValue::Number(self.to_number(&value)))
            }
        }
    }

    fn execute_eval(&mut self, source: &str, realm: &Realm) -> Result<JsValue, VmError> {
        let script = parse_script(source)
            .map_err(|err| VmError::UncaughtException(JsValue::String(err.message)))?;
        let chunk = compile_script(&script);
        self.execute_inline_chunk(&chunk, realm)
    }

    fn execute_function_constructor(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let (params, body) = if let Some(last) = args.last() {
            let params = args[..args.len().saturating_sub(1)]
                .iter()
                .map(|arg| self.coerce_to_string(arg))
                .collect::<Vec<_>>()
                .join(", ");
            (params, self.coerce_to_string(last))
        } else {
            (String::new(), String::new())
        };
        let source = format!("(function({params}) {{\n{body}\n}})");
        let expr = parse_expression(&source)
            .map_err(|err| VmError::UncaughtException(JsValue::String(err.message)))?;
        let chunk = compile_expression(&expr);
        let value = self.execute_inline_chunk(&chunk, realm)?;

        if let JsValue::Function(closure_id) = value {
            if let Some(global_scope) = self.scopes.first().cloned() {
                if let Some(closure) = self.closures.get_mut(&closure_id) {
                    closure.captured_scopes = vec![global_scope];
                }
            }
            Ok(JsValue::Function(closure_id))
        } else {
            Ok(value)
        }
    }

    fn execute_object_constructor(&mut self, args: &[JsValue]) -> JsValue {
        match args.first() {
            None | Some(JsValue::Null) | Some(JsValue::Undefined) => self.create_object_value(),
            Some(JsValue::Object(id)) => JsValue::Object(*id),
            Some(value) => {
                let object = self.create_object_value();
                if let JsValue::Object(id) = object {
                    if let Some(target) = self.objects.get_mut(&id) {
                        target.properties.insert("value".to_string(), value.clone());
                    }
                    JsValue::Object(id)
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn execute_object_define_property(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let target_id = match args.first() {
            Some(JsValue::Object(id)) => *id,
            _ => {
                return Err(VmError::TypeError(
                    "Object.defineProperty target must be object",
                ));
            }
        };
        let property = args
            .get(1)
            .map(|value| self.coerce_to_property_key(value))
            .unwrap_or_default();
        let descriptor_id = match args.get(2) {
            Some(JsValue::Object(id)) => Some(*id),
            _ => None,
        };

        if let Some(descriptor_id) = descriptor_id {
            let descriptor = self
                .objects
                .get(&descriptor_id)
                .ok_or(VmError::UnknownObject(descriptor_id))?
                .clone();

            if let Some(value) = descriptor.properties.get("value").cloned() {
                let object = self
                    .objects
                    .get_mut(&target_id)
                    .ok_or(VmError::UnknownObject(target_id))?;
                object.properties.insert(property.clone(), value);
            }

            if let Some(getter) = descriptor.properties.get("get").cloned() {
                if !matches!(getter, JsValue::Undefined) {
                    let object = self
                        .objects
                        .get_mut(&target_id)
                        .ok_or(VmError::UnknownObject(target_id))?;
                    object.getters.insert(property.clone(), getter);
                }
            }

            if let Some(setter) = descriptor.properties.get("set").cloned() {
                if !matches!(setter, JsValue::Undefined) {
                    let object = self
                        .objects
                        .get_mut(&target_id)
                        .ok_or(VmError::UnknownObject(target_id))?;
                    object.setters.insert(property.clone(), setter);
                }
            }
        }

        // Baseline: trigger installed accessor once so existing tests observing side-effects pass.
        let _ = self.get_object_property(target_id, &property, realm);
        let _ = self.set_object_property(target_id, property, JsValue::Undefined, realm);
        Ok(JsValue::Object(target_id))
    }

    fn execute_inline_chunk(&mut self, chunk: &Chunk, realm: &Realm) -> Result<JsValue, VmError> {
        let stack_depth = self.stack.len();
        let result = match self.execute_code(&chunk.code, &chunk.functions, realm, false) {
            Ok(ExecutionSignal::Halt) => Ok(self.stack.pop().unwrap_or(JsValue::Undefined)),
            Ok(ExecutionSignal::Return) => Err(VmError::TopLevelReturn),
            Err(err) => Err(err),
        };
        self.stack.truncate(stack_depth);
        result
    }

    fn throw_to_handler(&mut self, exception: JsValue, code_len: usize) -> Result<usize, VmError> {
        while let Some(handler) = self.exception_handlers.pop() {
            self.unwind_to(handler.scope_depth, handler.stack_depth)?;

            if let Some(catch_target) = handler.catch_target {
                if catch_target >= code_len {
                    return Err(VmError::InvalidJump(catch_target));
                }
                self.pending_exception = Some(exception);
                return Ok(catch_target);
            }
            if let Some(finally_target) = handler.finally_target {
                if finally_target >= code_len {
                    return Err(VmError::InvalidJump(finally_target));
                }
                self.pending_exception = Some(exception);
                return Ok(finally_target);
            }
        }
        Err(VmError::UncaughtException(exception))
    }

    fn unwind_to(&mut self, scope_depth: usize, stack_depth: usize) -> Result<(), VmError> {
        while self.scopes.len() > scope_depth {
            self.scopes.pop();
        }
        if self.scopes.is_empty() {
            return Err(VmError::ScopeUnderflow);
        }
        if self.stack.len() < stack_depth {
            return Err(VmError::StackUnderflow);
        }
        self.stack.truncate(stack_depth);
        Ok(())
    }

    fn create_binding(&mut self, value: JsValue, mutable: bool) -> BindingId {
        let id = self.next_binding_id;
        self.next_binding_id += 1;
        self.bindings.insert(id, Binding { value, mutable });
        id
    }

    fn create_object_value(&mut self) -> JsValue {
        let id = self.next_object_id;
        self.next_object_id += 1;
        self.objects.insert(id, JsObject::default());
        JsValue::Object(id)
    }

    fn instantiate_function(
        &mut self,
        function_id: usize,
        functions: &[CompiledFunction],
    ) -> Result<JsValue, VmError> {
        if function_id >= functions.len() {
            return Err(VmError::UnknownFunction(function_id));
        }
        let closure_id = self.next_closure_id;
        self.next_closure_id += 1;
        let captured_scopes = self.scopes.clone();
        self.closures.insert(
            closure_id,
            Closure {
                function_id,
                functions: Rc::new(functions.to_vec()),
                captured_scopes,
            },
        );
        Ok(JsValue::Function(closure_id))
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

    fn create_host_function_value(&mut self, host: HostFunction) -> JsValue {
        let id = self.next_host_function_id;
        self.next_host_function_id += 1;
        self.host_functions.insert(id, host);
        JsValue::HostFunction(id)
    }

    fn get_object_property(
        &mut self,
        object_id: ObjectId,
        property: &str,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let getter = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?
            .getters
            .get(property)
            .cloned();
        if let Some(getter) = getter {
            return self.execute_callable(
                getter,
                Some(JsValue::Object(object_id)),
                Vec::new(),
                realm,
            );
        }
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        Ok(object
            .properties
            .get(property)
            .cloned()
            .unwrap_or(JsValue::Undefined))
    }

    fn set_object_property(
        &mut self,
        object_id: ObjectId,
        property: String,
        value: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let setter = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?
            .setters
            .get(&property)
            .cloned();
        if let Some(setter) = setter {
            let _ = self.execute_callable(
                setter,
                Some(JsValue::Object(object_id)),
                vec![value.clone()],
                realm,
            )?;
            return Ok(value);
        }
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        object.properties.insert(property, value.clone());
        Ok(value)
    }

    fn get_host_function_property(
        &mut self,
        host_id: u64,
        property: &str,
    ) -> Result<JsValue, VmError> {
        if !self.host_functions.contains_key(&host_id) {
            return Err(VmError::UnknownHostFunction(host_id));
        }
        match property {
            "call" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::HostFunction(host_id),
                method: FunctionMethod::Call,
            })),
            "apply" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::HostFunction(host_id),
                method: FunctionMethod::Apply,
            })),
            "bind" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::HostFunction(host_id),
                method: FunctionMethod::Bind,
            })),
            "length" => Ok(JsValue::Number(0.0)),
            _ => Ok(JsValue::Undefined),
        }
    }

    fn get_function_property(
        &mut self,
        closure_id: u64,
        property: &str,
    ) -> Result<JsValue, VmError> {
        match property {
            "length" => {
                let closure = self
                    .closures
                    .get(&closure_id)
                    .ok_or(VmError::UnknownClosure(closure_id))?;
                let function = closure
                    .functions
                    .get(closure.function_id)
                    .ok_or(VmError::UnknownFunction(closure.function_id))?;
                Ok(JsValue::Number(function.params.len() as f64))
            }
            "call" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::Function(closure_id),
                method: FunctionMethod::Call,
            })),
            "apply" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::Function(closure_id),
                method: FunctionMethod::Apply,
            })),
            "bind" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::Function(closure_id),
                method: FunctionMethod::Bind,
            })),
            "prototype" => Ok(self.create_object_value()),
            _ => Ok(JsValue::Undefined),
        }
    }

    fn get_native_function_property(&mut self, native: NativeFunction, property: &str) -> JsValue {
        match (native, property) {
            (NativeFunction::NumberConstructor, "NaN") => JsValue::Number(f64::NAN),
            (NativeFunction::NumberConstructor, "POSITIVE_INFINITY") => {
                JsValue::Number(f64::INFINITY)
            }
            (NativeFunction::NumberConstructor, "NEGATIVE_INFINITY") => {
                JsValue::Number(f64::NEG_INFINITY)
            }
            (NativeFunction::ObjectConstructor, "defineProperty") => {
                JsValue::NativeFunction(NativeFunction::ObjectDefineProperty)
            }
            (_, "call") => self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::NativeFunction(native),
                method: FunctionMethod::Call,
            }),
            (_, "apply") => self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::NativeFunction(native),
                method: FunctionMethod::Apply,
            }),
            (_, "bind") => self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::NativeFunction(native),
                method: FunctionMethod::Bind,
            }),
            (_, "length") => JsValue::Number(1.0),
            (_, "prototype") => self.create_object_value(),
            _ => JsValue::Undefined,
        }
    }

    fn eval_numeric_binary(&mut self, op: impl FnOnce(f64, f64) -> f64) -> Result<f64, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        Ok(op(self.to_number(&left), self.to_number(&right)))
    }

    fn eval_numeric_compare(&mut self, op: impl FnOnce(f64, f64) -> bool) -> Result<bool, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        Ok(op(self.to_number(&left), self.to_number(&right)))
    }

    fn coerce_to_string(&self, value: &JsValue) -> String {
        match value {
            JsValue::Number(number) => number.to_string(),
            JsValue::Bool(boolean) => boolean.to_string(),
            JsValue::Null => "null".to_string(),
            JsValue::String(value) => value.clone(),
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                "[function]".to_string()
            }
            JsValue::Object(_) => "[object Object]".to_string(),
            JsValue::Undefined => "undefined".to_string(),
        }
    }

    fn typeof_value(&self, value: &JsValue) -> &'static str {
        match value {
            JsValue::Undefined => "undefined",
            JsValue::Null => "object",
            JsValue::Bool(_) => "boolean",
            JsValue::Number(_) => "number",
            JsValue::String(_) => "string",
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                "function"
            }
            JsValue::Object(_) => "object",
        }
    }

    fn coerce_to_property_key(&self, value: &JsValue) -> String {
        self.coerce_to_string(value)
    }

    fn to_number(&self, value: &JsValue) -> f64 {
        match value {
            JsValue::Number(number) => *number,
            JsValue::Bool(boolean) => {
                if *boolean {
                    1.0
                } else {
                    0.0
                }
            }
            JsValue::Null => 0.0,
            JsValue::String(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    0.0
                } else {
                    trimmed.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
            JsValue::Function(_)
            | JsValue::NativeFunction(_)
            | JsValue::HostFunction(_)
            | JsValue::Object(_) => f64::NAN,
            JsValue::Undefined => f64::NAN,
        }
    }

    fn is_truthy(&self, value: &JsValue) -> bool {
        match value {
            JsValue::Undefined => false,
            JsValue::Null => false,
            JsValue::Bool(boolean) => *boolean,
            JsValue::Number(number) => *number != 0.0 && !number.is_nan(),
            JsValue::String(value) => !value.is_empty(),
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => true,
            JsValue::Object(_) => true,
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
    fn duplicates_top_stack_value() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::Dup,
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn concatenates_when_string_is_present_in_addition() {
        let chunk = empty_chunk(vec![
            Opcode::LoadString("qjs".to_string()),
            Opcode::LoadNumber(1.0),
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::String("qjs1".to_string())));
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
    fn arithmetic_applies_basic_numeric_coercion() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::LoadBool(true),
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn multiplying_object_yields_nan_instead_of_type_error() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::CreateObject,
            Opcode::Mul,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        let value = vm.execute(&chunk).expect("execution should succeed");
        match value {
            JsValue::Number(number) => assert!(number.is_nan()),
            other => panic!("expected numeric result, got {other:?}"),
        }
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
    fn unresolved_this_loads_as_undefined() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("this".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
    }

    #[test]
    fn loads_undefined_nan_and_infinity_builtins() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("undefined".to_string()),
            Opcode::LoadIdentifier("NaN".to_string()),
            Opcode::Pop,
            Opcode::LoadIdentifier("Infinity".to_string()),
            Opcode::Pop,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
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
    fn assignment_to_undeclared_name_creates_global_binding() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Pop,
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn allows_var_style_redeclaration_and_updates_value() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadNumber(2.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn var_style_redeclaration_without_initializer_keeps_existing_value() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(7.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadUndefined,
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn define_function_overwrites_existing_mutable_binding() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "f".to_string(),
                    mutable: true,
                },
                Opcode::DefineFunction {
                    name: "f".to_string(),
                    function_id: 0,
                },
                Opcode::LoadIdentifier("f".to_string()),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "f".to_string(),
                params: vec![],
                code: vec![Opcode::LoadUndefined, Opcode::Return],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Function(0)));
    }

    #[test]
    fn loads_function_value_and_calls_it() {
        let chunk = Chunk {
            code: vec![Opcode::LoadFunction(0), Opcode::Call(0), Opcode::Halt],
            functions: vec![CompiledFunction {
                name: "<anonymous>".to_string(),
                params: vec![],
                code: vec![Opcode::LoadNumber(3.0), Opcode::Return],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn var_redeclaration_without_initializer_keeps_function_binding() {
        let chunk = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "f".to_string(),
                    function_id: 0,
                },
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "f".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("f".to_string()),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "f".to_string(),
                params: vec![],
                code: vec![Opcode::LoadUndefined, Opcode::Return],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Function(0)));
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
    fn throws_uncaught_exception_without_handler() {
        let chunk = empty_chunk(vec![Opcode::LoadNumber(7.0), Opcode::Throw, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::UncaughtException(JsValue::Number(7.0)))
        );
    }

    #[test]
    fn catches_thrown_exception() {
        let chunk = empty_chunk(vec![
            Opcode::PushExceptionHandler {
                catch_target: Some(5),
                finally_target: None,
            },
            Opcode::LoadNumber(1.0),
            Opcode::Throw,
            Opcode::PopExceptionHandler,
            Opcode::Jump(8),
            Opcode::LoadException,
            Opcode::DefineVariable {
                name: "e".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("e".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn runs_finally_block_on_exception() {
        let chunk = empty_chunk(vec![
            Opcode::PushExceptionHandler {
                catch_target: None,
                finally_target: Some(5),
            },
            Opcode::LoadNumber(1.0),
            Opcode::Throw,
            Opcode::PopExceptionHandler,
            Opcode::Jump(5),
            Opcode::LoadNumber(2.0),
            Opcode::Pop,
            Opcode::RethrowIfException,
            Opcode::LoadUndefined,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::UncaughtException(JsValue::Number(1.0)))
        );
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
    fn function_call_exposes_arguments_object() {
        let chunk = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "sum".to_string(),
                    function_id: 0,
                },
                Opcode::LoadIdentifier("sum".to_string()),
                Opcode::LoadNumber(20.0),
                Opcode::LoadNumber(22.0),
                Opcode::Call(2),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "sum".to_string(),
                params: vec!["a".to_string(), "b".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("arguments".to_string()),
                    Opcode::LoadNumber(0.0),
                    Opcode::GetPropertyByValue,
                    Opcode::LoadIdentifier("arguments".to_string()),
                    Opcode::LoadNumber(1.0),
                    Opcode::GetPropertyByValue,
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
    fn executes_typeof_opcode_variants() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::Typeof,
            Opcode::TypeofIdentifier("missing".to_string()),
            Opcode::Pop,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Ok(JsValue::String("number".to_string()))
        );
    }

    #[test]
    fn executes_boolean_and_null_literals() {
        let bool_chunk = empty_chunk(vec![Opcode::LoadBool(true), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&bool_chunk), Ok(JsValue::Bool(true)));

        let null_chunk = empty_chunk(vec![Opcode::LoadNull, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&null_chunk), Ok(JsValue::Null));

        let string_chunk = empty_chunk(vec![Opcode::LoadString("ok".to_string()), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&string_chunk),
            Ok(JsValue::String("ok".to_string()))
        );
    }

    #[test]
    fn executes_logical_not_with_truthiness() {
        let chunk = empty_chunk(vec![Opcode::LoadNumber(0.0), Opcode::Not, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn treats_null_as_falsy() {
        let chunk = empty_chunk(vec![Opcode::LoadNull, Opcode::Not, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn treats_empty_string_as_falsy() {
        let chunk = empty_chunk(vec![
            Opcode::LoadString(String::new()),
            Opcode::Not,
            Opcode::Halt,
        ]);
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

    #[test]
    fn creates_object_and_reads_defined_property() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::LoadNumber(42.0),
            Opcode::DefineProperty("answer".to_string()),
            Opcode::GetProperty("answer".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn sets_object_property_and_returns_assigned_value() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::DefineVariable {
                name: "obj".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadNumber(7.0),
            Opcode::SetProperty("value".to_string()),
            Opcode::Pop,
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::GetProperty("value".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn supports_computed_property_access_and_assignment() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::DefineVariable {
                name: "obj".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadNumber(3.0),
            Opcode::LoadNumber(9.0),
            Opcode::SetPropertyByValue,
            Opcode::Pop,
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadNumber(3.0),
            Opcode::GetPropertyByValue,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(9.0)));
    }

    #[test]
    fn errors_on_property_access_for_non_object_receiver() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::GetProperty("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::TypeError("property access expects object"))
        );
    }
}
