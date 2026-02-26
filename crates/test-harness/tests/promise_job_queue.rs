#![forbid(unsafe_code)]

use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use vm::{PromiseJobDrainReport, PromiseJobDrainStopReason, PromiseJobHostHooks, Vm, VmError};

#[derive(Default)]
struct RecordingHooks {
    events: Vec<String>,
    fail_on_enqueue: bool,
    fail_on_drain_start: bool,
    fail_on_drain_end: bool,
}

impl PromiseJobHostHooks for RecordingHooks {
    fn on_enqueue(&mut self, pending_jobs: usize) -> Result<(), VmError> {
        self.events.push(format!("enqueue:{pending_jobs}"));
        if self.fail_on_enqueue {
            return Err(VmError::RuntimeIntegrity("enqueue callback failed"));
        }
        Ok(())
    }

    fn on_drain_start(&mut self, pending_jobs: usize) -> Result<(), VmError> {
        self.events.push(format!("drain_start:{pending_jobs}"));
        if self.fail_on_drain_start {
            return Err(VmError::RuntimeIntegrity("drain_start callback failed"));
        }
        Ok(())
    }

    fn on_drain_end(&mut self, report: &PromiseJobDrainReport) -> Result<(), VmError> {
        self.events.push(format!(
            "drain_end:{}:{}:{:?}",
            report.processed, report.remaining, report.stop_reason
        ));
        if self.fail_on_drain_end {
            return Err(VmError::RuntimeIntegrity("drain_end callback failed"));
        }
        Ok(())
    }
}

fn execute_script(vm: &mut Vm, realm: &Realm, source: &str) -> JsValue {
    let script = parse_script(source).expect("script should parse");
    let chunk = compile_script(&script);
    vm.execute_in_realm(&chunk, realm)
        .expect("script should execute successfully")
}

#[test]
fn host_callbacks_cover_enqueue_and_bounded_drain() {
    let mut vm = Vm::default();
    let realm = Realm::default();
    let mut hooks = RecordingHooks::default();

    vm.enqueue_host_promise_job(JsValue::Number(1.0), &mut hooks)
        .expect("first host enqueue should succeed");
    vm.enqueue_host_promise_job(JsValue::Number(2.0), &mut hooks)
        .expect("second host enqueue should succeed");

    let report = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect("drain should succeed");
    assert_eq!(
        report,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 1,
            stop_reason: PromiseJobDrainStopReason::BudgetExhausted,
        }
    );
    assert_eq!(
        hooks.events,
        vec![
            "enqueue:1".to_string(),
            "enqueue:2".to_string(),
            "drain_start:2".to_string(),
            "drain_end:1:1:BudgetExhausted".to_string(),
        ]
    );
}

#[test]
fn nested_promise_reactions_enqueue_during_drain() {
    let mut realm = Realm::default();
    install_baseline(&mut realm);
    let mut vm = Vm::default();

    let result = execute_script(
        &mut vm,
        &realm,
        "var log = []; \
         async function base() { return 1; } \
         var p = base(); \
         var p2 = p.then(function(v) { log.push(v); return v + 1; }); \
         p2.then(function(v) { log.push(v); return v + 1; }); \
         log.length;",
    );
    assert_eq!(result, JsValue::Number(0.0));
    assert_eq!(vm.pending_promise_job_count(), 1);

    let mut hooks = RecordingHooks::default();
    let first = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect("first drain should process one job and enqueue tail work");
    assert_eq!(
        first,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 1,
            stop_reason: PromiseJobDrainStopReason::BudgetExhausted,
        }
    );
    let second = vm
        .drain_promise_jobs_with_host_hooks(8, &realm, false, &mut hooks)
        .expect("second drain should consume remaining nested work");
    assert_eq!(
        second,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 0,
            stop_reason: PromiseJobDrainStopReason::QueueEmpty,
        }
    );
    assert_eq!(
        hooks.events,
        vec![
            "drain_start:1".to_string(),
            "enqueue:1".to_string(),
            "drain_end:1:1:BudgetExhausted".to_string(),
            "drain_start:1".to_string(),
            "drain_end:1:0:QueueEmpty".to_string(),
        ]
    );
}

#[test]
fn host_callback_failures_return_deterministic_typed_errors() {
    let realm = Realm::default();

    let mut vm = Vm::default();
    let mut hooks = RecordingHooks {
        fail_on_enqueue: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .enqueue_host_promise_job(JsValue::Number(1.0), &mut hooks)
        .expect_err("enqueue callback failure should return typed error");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnEnqueueFailed")
    );

    let mut vm = Vm::default();
    vm.enqueue_host_promise_job(JsValue::Number(2.0), &mut RecordingHooks::default())
        .expect("queue setup should succeed");
    let mut hooks = RecordingHooks {
        fail_on_drain_start: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("drain-start callback failure should return typed error");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainStartFailed")
    );

    let mut vm = Vm::default();
    vm.enqueue_host_promise_job(JsValue::Number(3.0), &mut RecordingHooks::default())
        .expect("queue setup should succeed");
    let mut hooks = RecordingHooks {
        fail_on_drain_end: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("drain-end callback failure should return typed error");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainEndFailed")
    );
}
