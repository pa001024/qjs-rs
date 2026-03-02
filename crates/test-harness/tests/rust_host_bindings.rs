use runtime::JsValue;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use test_harness::{
    HostCallbackRegistration, execute_script_with_host_callbacks, run_script_with_host_callbacks,
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
