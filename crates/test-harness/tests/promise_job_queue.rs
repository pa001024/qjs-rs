#![forbid(unsafe_code)]

use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use std::collections::BTreeMap;
use vm::{ModuleHost, ModuleHostError};
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

fn evaluate_module_with_nested_promises() -> Vm {
    let mut host = InMemoryModuleHost::default().with_module(
        "entry.js",
        "async function base() { return 1; }\n\
         const first = base();\n\
         const second = first.then(function (value) { return value + 1; });\n\
         second.then(function (value) { return value + 1; });\n\
         export const promise_type = typeof Promise;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("module evaluation should succeed");
    assert_eq!(
        exports.get("promise_type"),
        Some(&JsValue::String("function".to_string()))
    );
    vm
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

#[test]
fn module_path_promise_queue_matrix() {
    let realm = Realm::default();

    let mut vm = evaluate_module_with_nested_promises();
    let mut hooks = RecordingHooks::default();
    assert_eq!(vm.pending_promise_job_count(), 1);
    let first = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect("first drain should process head job and enqueue nested work");
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
        .expect("second drain should consume nested work");
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

    let mut vm = evaluate_module_with_nested_promises();
    let mut hooks = RecordingHooks {
        fail_on_enqueue: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("enqueue callback failure should be typed in module path");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnEnqueueFailed")
    );

    let mut vm = evaluate_module_with_nested_promises();
    let mut hooks = RecordingHooks {
        fail_on_drain_start: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("drain-start callback failure should be typed in module path");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainStartFailed")
    );

    let mut vm = evaluate_module_with_nested_promises();
    let mut hooks = RecordingHooks {
        fail_on_drain_end: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("drain-end callback failure should be typed in module path");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainEndFailed")
    );
}
