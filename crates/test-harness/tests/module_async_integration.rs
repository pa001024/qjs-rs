#![forbid(unsafe_code)]

use runtime::{JsValue, Realm};
use std::collections::BTreeMap;
use vm::{
    ModuleHost, ModuleHostError, PromiseJobDrainReport, PromiseJobDrainStopReason,
    PromiseJobHostHooks, Vm, VmError,
};

#[derive(Debug, Default)]
struct InMemoryModuleHost {
    modules: BTreeMap<String, String>,
}

impl InMemoryModuleHost {
    fn with_module(mut self, key: &str, source: &str) -> Self {
        self.modules.insert(key.to_string(), source.to_string());
        self
    }
}

impl ModuleHost for InMemoryModuleHost {
    fn resolve(
        &mut self,
        referrer: Option<&str>,
        specifier: &str,
    ) -> Result<String, ModuleHostError> {
        if let Some(specifier) = specifier.strip_prefix("./") {
            if let Some(referrer) = referrer {
                if let Some((prefix, _)) = referrer.rsplit_once('/') {
                    return Ok(format!("{prefix}/{specifier}"));
                }
            }
            return Ok(specifier.to_string());
        }
        Ok(specifier.to_string())
    }

    fn load(&mut self, canonical_key: &str) -> Result<String, ModuleHostError> {
        self.modules
            .get(canonical_key)
            .cloned()
            .ok_or(ModuleHostError::LoadFailed)
    }
}

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

fn evaluate_module_with_async_chains() -> Vm {
    let mut host = InMemoryModuleHost::default().with_module(
        "entry.js",
        "async function base() { return 1; }\n\
         const first = base();\n\
         const second = first.then(function (value) { return value + 1; });\n\
         const third = second.catch(function (reason) { return reason; });\n\
         third.finally(function () { return 0; });\n\
         export const promise_type = typeof third.then;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("module async graph should evaluate");
    assert_eq!(
        exports.get("promise_type"),
        Some(&JsValue::String("function".to_string()))
    );
    vm
}

#[test]
fn module_then_catch_finally_ordering() {
    let mut vm = evaluate_module_with_async_chains();
    let realm = Realm::default();
    let mut hooks = RecordingHooks::default();

    assert_eq!(vm.pending_promise_job_count(), 1);
    let first = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect("first drain should process async chain head");
    assert_eq!(
        first,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 1,
            stop_reason: PromiseJobDrainStopReason::BudgetExhausted,
        }
    );
    let second = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect("second drain should process then/catch link");
    assert_eq!(
        second,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 1,
            stop_reason: PromiseJobDrainStopReason::BudgetExhausted,
        }
    );
    let tail = vm
        .drain_promise_jobs_with_host_hooks(8, &realm, false, &mut hooks)
        .expect("tail drain should process finally jobs");
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
    let realm = Realm::default();

    let mut vm = evaluate_module_with_async_chains();
    let mut hooks = RecordingHooks {
        fail_on_enqueue: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("nested enqueue callback failure should be typed");
    assert_eq!(err, VmError::TypeError("PromiseJobQueue:HostOnEnqueueFailed"));

    let mut vm = evaluate_module_with_async_chains();
    let mut hooks = RecordingHooks {
        fail_on_drain_start: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("drain-start callback failure should be typed");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainStartFailed")
    );

    let mut vm = evaluate_module_with_async_chains();
    let mut hooks = RecordingHooks {
        fail_on_drain_end: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("drain-end callback failure should be typed");
    assert_eq!(err, VmError::TypeError("PromiseJobQueue:HostOnDrainEndFailed"));
}
