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
