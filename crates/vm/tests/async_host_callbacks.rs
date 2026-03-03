#![forbid(unsafe_code)]

use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use std::thread;
use std::time::Duration;
use vm::{PromiseJobDrainStopReason, Vm, VmError};

fn compile(source: &str) -> bytecode::Chunk {
    let script = parse_script(source).expect("script should parse");
    compile_script(&script)
}

fn drain_until_idle(vm: &mut Vm, realm: &Realm) {
    for _ in 0..256 {
        if !vm.has_pending_promise_jobs() {
            break;
        }
        let report = vm
            .drain_promise_jobs(1024, realm, false)
            .expect("promise drain should succeed");
        if report.remaining == 0
            || matches!(
                report.stop_reason,
                PromiseJobDrainStopReason::PendingAsyncHostCallbacks
            )
        {
            continue;
        }
    }
}

#[test]
fn async_host_callback_supports_await_and_then_chain() {
    let mut realm = Realm::default();
    install_baseline(&mut realm);
    let mut vm = Vm::default();
    vm.define_global_async_host_callback(
        &realm,
        "rustAsyncAdd",
        2.0,
        false,
        |_vm, _this_arg, args, _realm, _strict| {
            let lhs = match args.first() {
                Some(JsValue::Number(value)) => *value,
                _ => 0.0,
            };
            let rhs = match args.get(1) {
                Some(JsValue::Number(value)) => *value,
                _ => 0.0,
            };
            Ok(async move {
                thread::sleep(Duration::from_millis(5));
                Ok(JsValue::Number(lhs + rhs))
            })
        },
    )
    .expect("async callback should register");

    let bootstrap = compile(
        "globalThis.__result = 0; \
         async function run() { return await rustAsyncAdd(20, 22); } \
         run().then(function(v) { globalThis.__result = v; });",
    );
    vm.execute_in_realm_persistent(&bootstrap, &realm)
        .expect("script should execute");
    drain_until_idle(&mut vm, &realm);

    let readback = vm
        .execute_in_realm_persistent(&compile("globalThis.__result;"), &realm)
        .expect("readback should execute");
    assert_eq!(readback, JsValue::Number(42.0));
}

#[test]
fn interrupt_rejects_pending_async_host_callback_without_hanging_state() {
    let mut realm = Realm::default();
    install_baseline(&mut realm);
    let mut vm = Vm::default();
    vm.define_global_async_host_callback(
        &realm,
        "rustAsyncSlow",
        0.0,
        false,
        |_vm, _this_arg, _args, _realm, _strict| {
            Ok(async move {
                thread::sleep(Duration::from_millis(50));
                Ok(JsValue::Number(1.0))
            })
        },
    )
    .expect("async callback should register");

    vm.execute_in_realm_persistent(&compile("globalThis.__p = rustAsyncSlow();"), &realm)
        .expect("promise bootstrap should execute");
    assert!(vm.has_pending_async_host_callbacks());

    vm.set_interrupt_poll_interval(1);
    vm.set_interrupt_handler(|| true);
    let err = vm
        .execute_in_realm_persistent(&compile("1 + 2;"), &realm)
        .expect_err("interrupt should abort execution");
    assert_eq!(err, VmError::Interrupted);
    vm.clear_interrupt_handler();

    assert_eq!(vm.pending_async_host_callback_count(), 0);
    vm.execute_in_realm_persistent(
        &compile(
            "globalThis.__reason = ''; \
             globalThis.__p.catch(function(err) { \
                globalThis.__reason = err && err.name ? err.name : typeof err; \
             });",
        ),
        &realm,
    )
    .expect("catch registration should execute");
    drain_until_idle(&mut vm, &realm);
    let reason = vm
        .execute_in_realm_persistent(&compile("globalThis.__reason;"), &realm)
        .expect("reason readback should execute");
    assert_eq!(reason, JsValue::String("TypeError".to_string()));
}
