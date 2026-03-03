use bytecode::compile_script;
use parser::parse_script;
use runtime::JsValue;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use test_harness::{
    AsyncHostCallbackRegistration, HostCallbackExecution, HostCallbackRegistration,
    execute_script_with_host_callbacks, execute_script_with_host_callbacks_and_async_callbacks,
    run_script_with_host_callbacks,
};
use vm::VmError;

#[test]
fn host_function_callback_is_callable_from_js() {
    let result = run_script_with_host_callbacks(
        "rustAdd(1, 2);",
        vec![HostCallbackRegistration::function(
            "rustAdd",
            2.0,
            |_vm, _this_arg, args, _realm, _strict| {
                let lhs = args.first().map_or(0.0, |value| match value {
                    JsValue::Number(value) => *value,
                    _ => 0.0,
                });
                let rhs = args.get(1).map_or(0.0, |value| match value {
                    JsValue::Number(value) => *value,
                    _ => 0.0,
                });
                Ok(JsValue::Number(lhs + rhs))
            },
        )],
    );
    assert_eq!(result, Ok(JsValue::Number(3.0)));
}

#[test]
fn host_constructor_binding_reclaims_opaque_big_object_after_scope() {
    struct RustBigObj {
        payload: Vec<u8>,
        drop_counter: Arc<AtomicUsize>,
    }

    impl Drop for RustBigObj {
        fn drop(&mut self) {
            let _ = self.payload.len();
            self.drop_counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    let drop_counter = Arc::new(AtomicUsize::new(0));
    let drop_counter_for_callback = Arc::clone(&drop_counter);
    let mut execution = execute_script_with_host_callbacks(
        "{ const a = new RustBigObj(); }",
        vec![HostCallbackRegistration::constructor(
            "RustBigObj",
            0.0,
            move |vm, this_arg, _args, _realm, _strict| {
                let this_obj =
                    this_arg.ok_or(VmError::TypeError("HostCallback:MissingConstructorThis"))?;
                vm.bind_opaque_data(
                    &this_obj,
                    RustBigObj {
                        payload: vec![0u8; 1024 * 1024],
                        drop_counter: Arc::clone(&drop_counter_for_callback),
                    },
                )?;
                Ok(this_obj)
            },
        )],
    )
    .expect("script should execute");

    assert_eq!(execution.value, JsValue::Undefined);
    assert_eq!(drop_counter.load(Ordering::SeqCst), 0);
    let _ = execution.vm.collect_garbage(&execution.realm);
    assert_eq!(drop_counter.load(Ordering::SeqCst), 1);
}

#[test]
fn host_constructable_callback_requires_new_and_receives_constructor_this() {
    let result = run_script_with_host_callbacks(
        "var instance = new RustCtor(); typeof instance === 'object' && instance !== null;",
        vec![HostCallbackRegistration::constructor(
            "RustCtor",
            0.0,
            |_vm, this_arg, _args, _realm, _strict| {
                let this_obj =
                    this_arg.ok_or(VmError::TypeError("HostCallback:MissingConstructorThis"))?;
                Ok(this_obj)
            },
        )],
    );

    assert_eq!(result, Ok(JsValue::Bool(true)));
}

#[test]
fn host_non_constructable_callback_rejects_new() {
    let result = run_script_with_host_callbacks(
        "var threw = false; try { new RustFn(); } catch (err) { threw = err instanceof TypeError; } threw;",
        vec![HostCallbackRegistration::function(
            "RustFn",
            0.0,
            |_vm, _this_arg, _args, _realm, _strict| Ok(JsValue::Undefined),
        )],
    );

    assert_eq!(result, Ok(JsValue::Bool(true)));
}

#[test]
fn host_constructor_prototype_fallback_restores_backlink_after_non_object_override() {
    let result = run_script_with_host_callbacks(
        "var first = new HostCtor(); \
         var firstOk = Object.getPrototypeOf(first).constructor === HostCtor; \
         HostCtor.prototype = 1; \
         var second = new HostCtor(); \
         var secondProto = Object.getPrototypeOf(second); \
         firstOk && secondProto !== Object.prototype && secondProto.constructor === HostCtor;",
        vec![HostCallbackRegistration::constructor(
            "HostCtor",
            0.0,
            |_vm, this_arg, _args, _realm, _strict| {
                let this_obj =
                    this_arg.ok_or(VmError::TypeError("HostCallback:MissingConstructorThis"))?;
                Ok(this_obj)
            },
        )],
    );

    assert_eq!(result, Ok(JsValue::Bool(true)));
}

#[test]
fn object_set_prototype_of_host_target_enforces_cycle_extensibility_and_same_value_noop() {
    let result = run_script_with_host_callbacks(
        "var base = {}; \
         Object.setPrototypeOf(HostCtor, base); \
         var cycle = false; \
         try { Object.setPrototypeOf(base, HostCtor); } catch (err) { cycle = err instanceof TypeError; } \
         Object.preventExtensions(HostCtor); \
         var blocked = false; \
         try { Object.setPrototypeOf(HostCtor, {}); } catch (err) { blocked = err instanceof TypeError; } \
         var sameValue = Object.setPrototypeOf(HostCtor, base) === HostCtor; \
         cycle && blocked && sameValue;",
        vec![HostCallbackRegistration::constructor(
            "HostCtor",
            0.0,
            |_vm, this_arg, _args, _realm, _strict| {
                let this_obj =
                    this_arg.ok_or(VmError::TypeError("HostCallback:MissingConstructorThis"))?;
                Ok(this_obj)
            },
        )],
    );

    assert_eq!(result, Ok(JsValue::Bool(true)));
}

fn drain_all_pending_jobs(execution: &mut HostCallbackExecution) {
    for _ in 0..256 {
        if !execution.vm.has_pending_promise_jobs() {
            break;
        }
        let report = execution
            .vm
            .drain_promise_jobs(1024, &execution.realm, false)
            .expect("promise drain should succeed");
        if report.remaining == 0 {
            break;
        }
    }
}

fn execute_persistent(execution: &mut HostCallbackExecution, source: &str) -> JsValue {
    let script = parse_script(source).expect("script should parse");
    let chunk = compile_script(&script);
    execution
        .vm
        .execute_in_realm_persistent(&chunk, &execution.realm)
        .expect("script should execute")
}

#[test]
fn async_host_callback_returns_promise_and_resolves_for_await() {
    let mut execution = execute_script_with_host_callbacks_and_async_callbacks(
        "globalThis.__async_value = 0; \
         globalThis.__is_promise = rustAsyncAdd(20, 22) instanceof Promise; \
         rustAsyncAdd(20, 22).then(function(v) { globalThis.__async_value = v; });",
        Vec::new(),
        vec![AsyncHostCallbackRegistration::function(
            "rustAsyncAdd",
            2.0,
            |_vm, _this_arg, args, _realm, _strict| {
                let lhs = args.first().map_or(0.0, |value| match value {
                    JsValue::Number(value) => *value,
                    _ => 0.0,
                });
                let rhs = args.get(1).map_or(0.0, |value| match value {
                    JsValue::Number(value) => *value,
                    _ => 0.0,
                });
                Ok(async move {
                    thread::sleep(Duration::from_millis(5));
                    Ok(JsValue::Number(lhs + rhs))
                })
            },
        )],
    )
    .expect("script should execute");

    drain_all_pending_jobs(&mut execution);
    let is_promise = execute_persistent(&mut execution, "globalThis.__is_promise;");
    let value = execute_persistent(&mut execution, "globalThis.__async_value;");
    assert_eq!(is_promise, JsValue::Bool(true));
    assert_eq!(value, JsValue::Number(42.0));
}

#[test]
fn async_host_callback_rejection_flows_to_catch_and_finally_in_order() {
    let mut execution = execute_script_with_host_callbacks_and_async_callbacks(
        "globalThis.__events = []; \
         rustAsyncReject() \
            .then(function() { globalThis.__events.push('then'); }) \
            .catch(function(err) { \
                var name = err && err.name ? err.name : typeof err; \
                globalThis.__events.push('catch:' + name); \
            }) \
            .finally(function() { globalThis.__events.push('finally'); });",
        Vec::new(),
        vec![AsyncHostCallbackRegistration::function(
            "rustAsyncReject",
            0.0,
            |_vm, _this_arg, _args, _realm, _strict| {
                Ok(async move {
                    thread::sleep(Duration::from_millis(5));
                    Err(VmError::TypeError("HostCallback:AsyncReject"))
                })
            },
        )],
    )
    .expect("script should execute");

    drain_all_pending_jobs(&mut execution);
    let events = execute_persistent(&mut execution, "globalThis.__events.join(',');");
    assert_eq!(
        events,
        JsValue::String("catch:TypeError,finally".to_string())
    );
}
