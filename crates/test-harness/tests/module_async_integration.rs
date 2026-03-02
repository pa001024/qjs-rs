#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::{ModuleEntryExecution, run_module_entry_with_vm};
use vm::{PromiseJobDrainReport, PromiseJobDrainStopReason, PromiseJobHostHooks, VmError};

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

fn evaluate_module_with_async_chains() -> ModuleEntryExecution {
    let execution = run_module_entry_with_vm(
        "entry.js",
        &[(
            "entry.js",
            "async function base() { return 1; }\n\
             const first = base();\n\
             const second = first.then(function (value) { return value + 1; });\n\
             const third = second.catch(function (reason) { return reason; });\n\
             third.finally(function () { return 0; });\n\
             export const promise_type = typeof third.then;\n",
        )],
    )
    .expect("module async graph should evaluate");
    assert_eq!(
        execution.exports.get("promise_type"),
        Some(&JsValue::String("function".to_string()))
    );
    execution
}

#[test]
fn module_then_catch_finally_ordering() {
    let mut execution = evaluate_module_with_async_chains();
    let mut hooks = RecordingHooks::default();

    assert_eq!(execution.pending_promise_job_count(), 1);
    let first = execution
        .drain_promise_jobs_with_host_hooks(1, false, &mut hooks)
        .expect("first drain should process async chain head");
    assert_eq!(
        first,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 1,
            stop_reason: PromiseJobDrainStopReason::BudgetExhausted,
        }
    );
    let second = execution
        .drain_promise_jobs_with_host_hooks(1, false, &mut hooks)
        .expect("second drain should process then/catch link");
    assert_eq!(
        second,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 1,
            stop_reason: PromiseJobDrainStopReason::BudgetExhausted,
        }
    );
    let tail = execution
        .drain_promise_jobs_with_host_hooks(8, false, &mut hooks)
        .expect("tail drain should process finally job");
    assert_eq!(
        tail,
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
            "enqueue:1".to_string(),
            "drain_end:1:1:BudgetExhausted".to_string(),
            "drain_start:1".to_string(),
            "drain_end:1:0:QueueEmpty".to_string(),
        ]
    );
}

#[test]
fn module_host_hook_visibility() {
    let mut execution = evaluate_module_with_async_chains();
    let mut hooks = RecordingHooks {
        fail_on_enqueue: true,
        ..RecordingHooks::default()
    };
    let err = execution
        .drain_promise_jobs_with_host_hooks(1, false, &mut hooks)
        .expect_err("nested enqueue callback failure should be typed");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnEnqueueFailed")
    );

    let mut execution = evaluate_module_with_async_chains();
    let mut hooks = RecordingHooks {
        fail_on_drain_start: true,
        ..RecordingHooks::default()
    };
    let err = execution
        .drain_promise_jobs_with_host_hooks(1, false, &mut hooks)
        .expect_err("drain-start callback failure should be typed");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainStartFailed")
    );

    let mut execution = evaluate_module_with_async_chains();
    let mut hooks = RecordingHooks {
        fail_on_drain_end: true,
        ..RecordingHooks::default()
    };
    let err = execution
        .drain_promise_jobs_with_host_hooks(1, false, &mut hooks)
        .expect_err("drain-end callback failure should be typed");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainEndFailed")
    );
}
