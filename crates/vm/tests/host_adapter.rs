#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::JsValue;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use vm::{
    BoaLikeHostAdapter, ConsoleLevel, ConsoleLogger, HostClassAsyncMethodRegistration,
    HostClassMethodRegistration, HostClassRegistration, HostClassStaticMethodRegistration,
    ScriptRuntime, VmError,
};

#[derive(Debug)]
struct Counter {
    value: f64,
}

#[test]
fn host_class_sync_methods() {
    let mut runtime = ScriptRuntime::new();
    runtime
        .register_host_class(
            HostClassRegistration::new("Counter", 1.0, |args, _realm, _strict| {
                let initial = match args.first() {
                    Some(JsValue::Number(value)) => *value,
                    _ => 0.0,
                };
                Ok(Counter { value: initial })
            })
            .with_method(HostClassMethodRegistration::new(
                "add",
                1.0,
                |instance: &mut Counter, args| {
                    let delta = match args.first() {
                        Some(JsValue::Number(value)) => *value,
                        _ => 0.0,
                    };
                    instance.value += delta;
                    Ok(JsValue::Number(instance.value))
                },
            ))
            .with_static_method(HostClassStaticMethodRegistration::new("id", 1.0, |args| {
                Ok(args.first().cloned().unwrap_or(JsValue::Undefined))
            })),
        )
        .expect("host class should register");

    let result = runtime
        .execute_source(
            "let threw = false; \
             try { Counter(1); } catch (err) { threw = err instanceof TypeError; } \
             const c = new Counter(2); \
             const v1 = c.add(3); \
             const v2 = c.add(4); \
             threw && v1 === 5 && v2 === 9 && Counter.id(7) === 7;",
        )
        .expect("script should execute");
    assert_eq!(result.value, JsValue::Bool(true));
}

#[derive(Debug)]
struct AsyncCounter {
    value: f64,
}

#[test]
fn host_class_async_methods() {
    let mut runtime = ScriptRuntime::new();
    runtime
        .register_host_class(
            HostClassRegistration::new("AsyncCounter", 1.0, |args, _realm, _strict| {
                let initial = match args.first() {
                    Some(JsValue::Number(value)) => *value,
                    _ => 0.0,
                };
                Ok(AsyncCounter { value: initial })
            })
            .with_async_method(HostClassAsyncMethodRegistration::new(
                "addAsync",
                1.0,
                |instance: &mut AsyncCounter, args| {
                    let delta = match args.first() {
                        Some(JsValue::Number(value)) => *value,
                        _ => 0.0,
                    };
                    instance.value += delta;
                    let next = instance.value;
                    Ok(async move {
                        thread::sleep(Duration::from_millis(5));
                        Ok(JsValue::Number(next))
                    })
                },
            )),
        )
        .expect("async host class should register");

    runtime
        .execute_source(
            "globalThis.__async_counter_result = 0; \
             new AsyncCounter(10).addAsync(5).then(function(v) { globalThis.__async_counter_result = v; });",
        )
        .expect("bootstrap should execute");
    let read_back = runtime
        .execute_source("globalThis.__async_counter_result;")
        .expect("readback should execute");
    assert_eq!(read_back.value, JsValue::Number(15.0));
}

#[derive(Debug)]
struct SlowClass;

#[test]
fn interrupt_rejects_pending_class_async_promises() {
    let mut runtime = ScriptRuntime::new();
    runtime
        .register_host_class(
            HostClassRegistration::new("SlowClass", 0.0, |_args, _realm, _strict| Ok(SlowClass))
                .with_async_method(HostClassAsyncMethodRegistration::new(
                    "slow",
                    0.0,
                    |_instance, _args| {
                        Ok(async move {
                            thread::sleep(Duration::from_millis(200));
                            Ok(JsValue::Number(1.0))
                        })
                    },
                )),
        )
        .expect("host class should register");

    let stop = Arc::new(AtomicBool::new(false));
    runtime.set_stop_token(Arc::clone(&stop));
    let setup_script = parse_script("globalThis.__pending = new SlowClass().slow();")
        .expect("script should parse");
    let setup_chunk = compile_script(&setup_script);
    let realm = runtime.realm().clone();
    runtime
        .vm_mut()
        .execute_in_realm_persistent(&setup_chunk, &realm)
        .expect("pending promise should be created");

    stop.store(true, Ordering::SeqCst);
    let err = runtime
        .execute_source("1 + 2;")
        .expect_err("interrupt should abort execution");
    assert!(matches!(
        err,
        vm::ScriptRuntimeError::Vm(VmError::Interrupted)
    ));
    assert_eq!(runtime.vm().pending_async_host_callback_count(), 0);

    stop.store(false, Ordering::SeqCst);
    let state = runtime
        .execute_source("globalThis.__pending.__asyncState;")
        .expect("pending state should be readable");
    assert_eq!(state.value, JsValue::String("rejected".to_string()));
    runtime
        .execute_source(
            "globalThis.__pending_reason = ''; \
             globalThis.__pending.catch(function(err) { \
               globalThis.__pending_reason = err && err.name ? err.name : typeof err; \
             });",
        )
        .expect("catch script should execute");
    for _ in 0..4 {
        let _ = runtime.drain_jobs(1024).expect("drain should succeed");
    }
    let reason = runtime
        .execute_source("globalThis.__pending_reason;")
        .expect("reason readback should execute");
    assert_eq!(reason.value, JsValue::String("TypeError".to_string()));
}

#[derive(Clone)]
struct RecordingLogger {
    entries: Rc<RefCell<Vec<String>>>,
}

impl ConsoleLogger for RecordingLogger {
    fn on_console(&mut self, level: ConsoleLevel, args: &[JsValue]) {
        let level_text = match level {
            ConsoleLevel::Log => "log",
            ConsoleLevel::Info => "info",
            ConsoleLevel::Warn => "warn",
            ConsoleLevel::Error => "error",
            ConsoleLevel::Debug => "debug",
        };
        let payload = args
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join("|");
        self.entries
            .borrow_mut()
            .push(format!("{level_text}:{payload}"));
    }
}

#[test]
fn console_logger_bridge() {
    let mut runtime = ScriptRuntime::new();
    let entries = Rc::new(RefCell::new(Vec::new()));
    runtime
        .inject_console_logger(RecordingLogger {
            entries: Rc::clone(&entries),
        })
        .expect("console should inject");

    runtime
        .execute_source(
            "console.log('alpha', 1); \
             console.info('beta'); \
             console.warn('gamma'); \
             console.error('delta'); \
             console.debug('epsilon');",
        )
        .expect("console script should execute");

    let captured = entries.borrow().clone();
    assert_eq!(captured.len(), 5);
    assert!(captured[0].starts_with("log:"));
    assert!(captured[1].starts_with("info:"));
    assert!(captured[2].starts_with("warn:"));
    assert!(captured[3].starts_with("error:"));
    assert!(captured[4].starts_with("debug:"));
}

#[derive(Debug)]
struct HostCtor {
    value: f64,
}

#[test]
fn boa_like_adapter_smoke() {
    let mut adapter = BoaLikeHostAdapter::new();
    adapter
        .register_global_function("rustAdd", 2.0, |_vm, _this_arg, args, _realm, _strict| {
            let lhs = match args.first() {
                Some(JsValue::Number(value)) => *value,
                _ => 0.0,
            };
            let rhs = match args.get(1) {
                Some(JsValue::Number(value)) => *value,
                _ => 0.0,
            };
            Ok(JsValue::Number(lhs + rhs))
        })
        .expect("global function should register");
    adapter
        .register_global_async_function(
            "rustAsyncAdd",
            2.0,
            |_vm, _this_arg, args, _realm, _strict| {
                let lhs = match args.first() {
                    Some(JsValue::Number(value)) => *value,
                    _ => 0.0,
                };
                let rhs = match args.get(1) {
                    Some(JsValue::Number(value)) => *value,
                    _ => 0.0,
                };
                Ok(async move { Ok(JsValue::Number(lhs + rhs)) })
            },
        )
        .expect("async function should register");
    adapter
        .register_host_class(
            HostClassRegistration::new("HostCtor", 1.0, |args, _realm, _strict| {
                let initial = match args.first() {
                    Some(JsValue::Number(value)) => *value,
                    _ => 0.0,
                };
                Ok(HostCtor { value: initial })
            })
            .with_method(HostClassMethodRegistration::new(
                "inc",
                1.0,
                |instance: &mut HostCtor, args| {
                    let delta = match args.first() {
                        Some(JsValue::Number(value)) => *value,
                        _ => 0.0,
                    };
                    instance.value += delta;
                    Ok(JsValue::Number(instance.value))
                },
            )),
        )
        .expect("host class should register");

    adapter
        .run_script_source(
            "globalThis.__adapter_result = 0; \
             const host = new HostCtor(5); \
             rustAsyncAdd(rustAdd(1, 2), host.inc(3)) \
               .then(function(v) { globalThis.__adapter_result = v; });",
        )
        .expect("bootstrap should execute");
    let result = adapter
        .run_script_source("globalThis.__adapter_result;")
        .expect("readback should execute");
    assert_eq!(result.value, JsValue::Number(11.0));
}
