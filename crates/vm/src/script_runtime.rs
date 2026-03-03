use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Vm, VmError};

type ScriptHostCallback =
    dyn FnMut(&mut Vm, Option<JsValue>, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>;

pub struct ScriptHostCallbackRegistration {
    pub name: String,
    pub length: f64,
    pub constructable: bool,
    pub callback: Box<ScriptHostCallback>,
}

impl ScriptHostCallbackRegistration {
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

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptRunOutput {
    pub value: JsValue,
    pub result_text: String,
    pub drained_promise_jobs: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptRuntimeError {
    Io(String),
    Parse(String),
    Vm(VmError),
}

impl fmt::Display for ScriptRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "{message}"),
            Self::Parse(message) => write!(f, "{message}"),
            Self::Vm(error) => write!(f, "{error:?}"),
        }
    }
}

impl Error for ScriptRuntimeError {}

impl From<VmError> for ScriptRuntimeError {
    fn from(value: VmError) -> Self {
        Self::Vm(value)
    }
}

pub struct ScriptRuntime {
    vm: Vm,
    realm: Realm,
}

impl Default for ScriptRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptRuntime {
    pub fn new() -> Self {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        Self {
            vm: Vm::default(),
            realm,
        }
    }

    pub fn vm(&self) -> &Vm {
        &self.vm
    }

    pub fn vm_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }

    pub fn realm(&self) -> &Realm {
        &self.realm
    }

    pub fn realm_mut(&mut self) -> &mut Realm {
        &mut self.realm
    }

    pub fn register_host_callback(
        &mut self,
        registration: ScriptHostCallbackRegistration,
    ) -> Result<JsValue, ScriptRuntimeError> {
        let ScriptHostCallbackRegistration {
            name,
            length,
            constructable,
            mut callback,
        } = registration;
        let value = self
            .vm
            .define_global_host_callback(
                &self.realm,
                &name,
                length,
                constructable,
                move |vm, this_arg, args, realm, caller_strict| {
                    callback(vm, this_arg, args, realm, caller_strict)
                },
            )
            .map_err(ScriptRuntimeError::Vm)?;
        Ok(value)
    }

    pub fn execute_source(&mut self, source: &str) -> Result<ScriptRunOutput, ScriptRuntimeError> {
        let script = parse_script(source).map_err(|err| ScriptRuntimeError::Parse(err.message))?;
        let chunk = compile_script(&script);
        let value = self
            .vm
            .execute_in_realm_persistent(&chunk, &self.realm)
            .map_err(ScriptRuntimeError::Vm)?;
        let drained_promise_jobs = self.drain_all_promise_jobs()?;
        let result_text = script_result_text(&value);
        Ok(ScriptRunOutput {
            value,
            result_text,
            drained_promise_jobs,
        })
    }

    pub fn execute_file<P>(&mut self, script_path: P) -> Result<ScriptRunOutput, ScriptRuntimeError>
    where
        P: AsRef<Path>,
    {
        let normalized = normalize_script_path(script_path)?;
        let source = fs::read_to_string(&normalized).map_err(|err| {
            ScriptRuntimeError::Io(format!(
                "读取脚本失败：{}，错误信息：{err}",
                normalized.display()
            ))
        })?;
        self.execute_source(&source)
    }

    fn drain_all_promise_jobs(&mut self) -> Result<usize, ScriptRuntimeError> {
        let mut drained = 0usize;
        while self.vm.has_pending_promise_jobs() {
            let report = self
                .vm
                .drain_promise_jobs(usize::MAX, &self.realm, false)
                .map_err(ScriptRuntimeError::Vm)?;
            if report.processed == 0 {
                break;
            }
            drained = drained.saturating_add(report.processed);
        }
        Ok(drained)
    }
}

pub fn normalize_script_path<P>(script_path: P) -> Result<PathBuf, ScriptRuntimeError>
where
    P: AsRef<Path>,
{
    let path = script_path.as_ref();
    if !path.exists() {
        return Err(ScriptRuntimeError::Io(format!(
            "脚本文件不存在：{}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(ScriptRuntimeError::Io(format!(
            "脚本路径不是文件：{}",
            path.display()
        )));
    }
    path.canonicalize().map_err(|err| {
        ScriptRuntimeError::Io(format!(
            "规范化脚本路径失败：{}，错误信息：{err}",
            path.display()
        ))
    })
}

pub fn script_result_text(value: &JsValue) -> String {
    match value {
        JsValue::Undefined | JsValue::Null => String::new(),
        JsValue::String(text) => text.clone(),
        JsValue::Bool(flag) => flag.to_string(),
        JsValue::Number(number) => format_js_number(*number),
        _ => format!("{value:?}"),
    }
}

fn format_js_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    let mut rendered = value.to_string();
    if rendered.ends_with(".0") {
        rendered.truncate(rendered.len() - 2);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::{normalize_script_path, ScriptHostCallbackRegistration, ScriptRuntime};
    use runtime::JsValue;
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn executes_source_and_returns_text_result() {
        let mut runtime = ScriptRuntime::new();
        let result = runtime
            .execute_source("1 + 2;")
            .expect("script source should execute");
        assert_eq!(result.value, JsValue::Number(3.0));
        assert_eq!(result.result_text, "3");
    }

    #[test]
    fn executes_file_with_normalized_path() {
        let script_name = format!(
            "qjs-rs-script-runtime-{}.js",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be monotonic")
                .as_nanos()
        );
        let script_path = env::temp_dir().join(script_name);
        fs::write(&script_path, "40 + 2;").expect("temporary script should be written");

        let mut runtime = ScriptRuntime::new();
        let result = runtime
            .execute_file(&script_path)
            .expect("script file should execute");
        assert_eq!(result.value, JsValue::Number(42.0));
        assert_eq!(result.result_text, "42");

        fs::remove_file(&script_path).expect("temporary script should be removed");
    }

    #[test]
    fn supports_registered_host_callbacks() {
        let mut runtime = ScriptRuntime::new();
        runtime
            .register_host_callback(ScriptHostCallbackRegistration::function(
                "rustAdd",
                2.0,
                |_vm, _this_arg, args, _realm, _strict| {
                    let lhs = args.first().cloned().unwrap_or(JsValue::Number(0.0));
                    let rhs = args.get(1).cloned().unwrap_or(JsValue::Number(0.0));
                    let lhs = match lhs {
                        JsValue::Number(value) => value,
                        _ => 0.0,
                    };
                    let rhs = match rhs {
                        JsValue::Number(value) => value,
                        _ => 0.0,
                    };
                    Ok(JsValue::Number(lhs + rhs))
                },
            ))
            .expect("host callback should register");

        let result = runtime
            .execute_source("rustAdd(20, 22);")
            .expect("script with host callback should execute");
        assert_eq!(result.value, JsValue::Number(42.0));
        assert_eq!(result.result_text, "42");
    }

    #[test]
    fn drains_promise_jobs_after_execution() {
        let mut runtime = ScriptRuntime::new();
        let result = runtime
            .execute_source(
                "async function __qjs_script_runner_base(v) { return v; } \
                 globalThis.__qjs_script_runner_value = 0; \
                 __qjs_script_runner_base(7).then(function(v) { globalThis.__qjs_script_runner_value = v; });",
            )
            .expect("script should execute");
        assert!(result.drained_promise_jobs > 0);

        let read_back = runtime
            .execute_source("globalThis.__qjs_script_runner_value;")
            .expect("readback should execute");
        assert_eq!(read_back.value, JsValue::Number(7.0));
    }

    #[test]
    fn supports_promise_static_methods_baseline() {
        let mut runtime = ScriptRuntime::new();
        let bootstrap = runtime
            .execute_source(
                "globalThis.__promise_all = 0; \
                 globalThis.__promise_any = 0; \
                 globalThis.__promise_race = 0; \
                 globalThis.__promise_reject = 0; \
                 globalThis.__promise_thenable = 0; \
                 globalThis.__promise_all_settled = ''; \
                 Promise.all([Promise.resolve(1), 2, Promise.resolve(3)]) \
                    .then(function(values) { globalThis.__promise_all = values[0] + values[1] + values[2]; }); \
                 Promise.any([Promise.reject(8), Promise.resolve(6)]) \
                    .then(function(value) { globalThis.__promise_any = value; }); \
                 Promise.race([Promise.resolve(5), Promise.resolve(9)]) \
                    .then(function(value) { globalThis.__promise_race = value; }); \
                 Promise.reject(4) \
                    .catch(function(reason) { globalThis.__promise_reject = reason; }); \
                 Promise.resolve({ then: function(resolve) { resolve(11); } }) \
                    .then(function(value) { globalThis.__promise_thenable = value; }); \
                 Promise.allSettled([Promise.resolve(1), Promise.reject(2)]) \
                    .then(function(values) { globalThis.__promise_all_settled = values[0].status + ':' + values[1].status; });",
            )
            .expect("promise static bootstrap should execute");
        assert!(bootstrap.drained_promise_jobs > 0);

        let all = runtime
            .execute_source("globalThis.__promise_all;")
            .expect("all readback should execute");
        assert_eq!(all.value, JsValue::Number(6.0));

        let race = runtime
            .execute_source("globalThis.__promise_race;")
            .expect("race readback should execute");
        assert_eq!(race.value, JsValue::Number(5.0));

        let any = runtime
            .execute_source("globalThis.__promise_any;")
            .expect("any readback should execute");
        assert_eq!(any.value, JsValue::Number(6.0));

        let reject = runtime
            .execute_source("globalThis.__promise_reject;")
            .expect("reject readback should execute");
        assert_eq!(reject.value, JsValue::Number(4.0));

        let thenable = runtime
            .execute_source("globalThis.__promise_thenable;")
            .expect("thenable readback should execute");
        assert_eq!(thenable.value, JsValue::Number(11.0));

        let all_settled = runtime
            .execute_source("globalThis.__promise_all_settled;")
            .expect("allSettled readback should execute");
        assert_eq!(
            all_settled.value,
            JsValue::String("fulfilled:rejected".to_string())
        );
    }

    #[test]
    fn normalize_script_path_rejects_missing_file() {
        let missing = env::temp_dir().join("qjs-rs-script-runtime-missing.js");
        let error = normalize_script_path(&missing).expect_err("missing file should fail");
        let message = error.to_string();
        assert!(message.contains("脚本文件不存在"));
    }
}
