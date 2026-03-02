use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use vm::{Vm, VmError};

type HostCallback =
    dyn FnMut(&mut Vm, Option<JsValue>, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>;

pub struct HostCallbackRegistration {
    pub name: String,
    pub length: f64,
    pub constructable: bool,
    pub callback: Box<HostCallback>,
}

impl HostCallbackRegistration {
    pub fn function<F>(name: impl Into<String>, length: f64, callback: F) -> Self
    where
        F: FnMut(&mut Vm, Option<JsValue>, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>
            + 'static,
    {
        Self {
            name: name.into(),
            length,
            constructable: false,
            callback: Box::new(callback),
        }
    }

    pub fn constructor<F>(name: impl Into<String>, length: f64, callback: F) -> Self
    where
        F: FnMut(&mut Vm, Option<JsValue>, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>
            + 'static,
    {
        Self {
            name: name.into(),
            length,
            constructable: true,
            callback: Box::new(callback),
        }
    }
}

pub struct HostCallbackExecution {
    pub value: JsValue,
    pub vm: Vm,
    pub realm: Realm,
}

pub fn run_script_with_host_callbacks(
    source: &str,
    callbacks: Vec<HostCallbackRegistration>,
) -> Result<JsValue, String> {
    execute_script_with_host_callbacks(source, callbacks).map(|execution| execution.value)
}

pub fn execute_script_with_host_callbacks(
    source: &str,
    callbacks: Vec<HostCallbackRegistration>,
) -> Result<HostCallbackExecution, String> {
    let script = parse_script(source).map_err(|err| err.message)?;
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    install_baseline(&mut realm);
    let mut vm = Vm::default();

    for callback in callbacks {
        let HostCallbackRegistration {
            name,
            length,
            constructable,
            mut callback,
        } = callback;
        vm.define_global_host_callback(
            &realm,
            &name,
            length,
            constructable,
            move |vm, this_arg, args, realm, caller_strict| {
                callback(vm, this_arg, args, realm, caller_strict)
            },
        )
        .map_err(|err| format!("{err:?}"))?;
    }

    let value = vm
        .execute_in_realm_persistent(&chunk, &realm)
        .map_err(|err| format!("{err:?}"))?;

    Ok(HostCallbackExecution { value, vm, realm })
}
