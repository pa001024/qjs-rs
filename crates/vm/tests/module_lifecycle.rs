#![forbid(unsafe_code)]

use runtime::{JsValue, ModuleLifecycleState, Realm};
use std::collections::{BTreeMap, BTreeSet};
use vm::{
    ModuleHost, ModuleHostError, PromiseJobDrainReport, PromiseJobDrainStopReason,
    PromiseJobHostHooks, Vm, VmError,
};

#[derive(Debug, Default)]
struct MemoryModuleHost {
    modules: BTreeMap<String, String>,
    load_counts: BTreeMap<String, usize>,
    fail_resolve: bool,
    resolve_empty: bool,
    fail_load_keys: BTreeSet<String>,
}

impl MemoryModuleHost {
    fn with_module(mut self, key: &str, source: &str) -> Self {
        self.modules.insert(key.to_string(), source.to_string());
        self
    }

    fn with_failing_load(mut self, key: &str) -> Self {
        self.fail_load_keys.insert(key.to_string());
        self
    }

    fn load_count(&self, key: &str) -> usize {
        self.load_counts.get(key).copied().unwrap_or_default()
    }
}

impl ModuleHost for MemoryModuleHost {
    fn resolve(
        &mut self,
        referrer: Option<&str>,
        specifier: &str,
    ) -> Result<String, ModuleHostError> {
        if self.fail_resolve {
            return Err(ModuleHostError::ResolveFailed);
        }
        if self.resolve_empty {
            return Ok(String::new());
        }
        Ok(canonicalize_module_specifier(referrer, specifier))
    }

    fn load(&mut self, canonical_key: &str) -> Result<String, ModuleHostError> {
        *self
            .load_counts
            .entry(canonical_key.to_string())
            .or_insert(0) += 1;
        if self.fail_load_keys.contains(canonical_key) {
            return Err(ModuleHostError::LoadFailed);
        }
        self.modules
            .get(canonical_key)
            .cloned()
            .ok_or(ModuleHostError::LoadFailed)
    }
}

fn canonicalize_module_specifier(referrer: Option<&str>, specifier: &str) -> String {
    if let Some(specifier) = specifier.strip_prefix("./") {
        if let Some(referrer) = referrer {
            if let Some((prefix, _)) = referrer.rsplit_once('/') {
                return format!("{prefix}/{specifier}");
            }
        }
        return specifier.to_string();
    }
    specifier.to_string()
}

fn load_number_export(exports: &BTreeMap<String, JsValue>, name: &str) -> f64 {
    let value = exports.get(name).cloned().unwrap_or(JsValue::Undefined);
    match value {
        JsValue::Number(number) => number,
        other => panic!("expected numeric export {name}, got {other:?}"),
    }
}

fn load_string_export(exports: &BTreeMap<String, JsValue>, name: &str) -> String {
    let value = exports.get(name).cloned().unwrap_or(JsValue::Undefined);
    match value {
        JsValue::String(text) => text,
        other => panic!("expected string export {name}, got {other:?}"),
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

fn evaluate_module_with_nested_promise_chain() -> Vm {
    let mut host = MemoryModuleHost::default().with_module(
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
    assert_eq!(load_string_export(&exports, "promise_type"), "function");
    vm
}

#[test]
fn module_state_transition_guards() {
    let mut host =
        MemoryModuleHost::default().with_module("entry.js", "export const answer = 42;\n");
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("module evaluation should succeed");
    assert_eq!(load_number_export(&exports, "answer"), 42.0);
    assert_eq!(
        vm.module_state("entry.js"),
        Some(ModuleLifecycleState::Evaluated)
    );

    let err = vm
        .debug_transition_module_state("entry.js", ModuleLifecycleState::Linking)
        .expect_err("invalid back-transition should fail deterministically");
    assert_eq!(err, VmError::TypeError("ModuleLifecycle:InvalidTransition"));

    let missing = vm
        .debug_transition_module_state("missing.js", ModuleLifecycleState::Linking)
        .expect_err("unknown module key should fail");
    assert_eq!(
        missing,
        VmError::TypeError("ModuleLifecycle:HostContractViolation")
    );
}

#[test]
fn module_cache_reuse_semantics() {
    let mut host = MemoryModuleHost::default()
        .with_module(
            "entry.js",
            "import { inc } from './dep.js';\nexport const value = inc + 1;\n",
        )
        .with_module("dep.js", "export const inc = 41;\n");
    let mut vm = Vm::default();

    let first = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("first module evaluation should succeed");
    let second = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("second module evaluation should reuse cache");

    assert_eq!(load_number_export(&first, "value"), 42.0);
    assert_eq!(load_number_export(&second, "value"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
    assert_eq!(vm.module_evaluation_count("entry.js"), Some(1));
    assert_eq!(vm.module_evaluation_count("dep.js"), Some(1));
}

#[test]
fn module_host_contract() {
    let mut resolve_fail_host = MemoryModuleHost {
        fail_resolve: true,
        ..MemoryModuleHost::default()
    };
    let mut vm = Vm::default();
    let resolve_err = vm
        .evaluate_module_entry("entry.js", &mut resolve_fail_host)
        .expect_err("resolve failure should map to deterministic type error");
    assert_eq!(
        resolve_err,
        VmError::TypeError("ModuleLifecycle:ResolveFailed")
    );

    let mut resolve_empty_host = MemoryModuleHost {
        resolve_empty: true,
        ..MemoryModuleHost::default()
    };
    let mut vm = Vm::default();
    let contract_err = vm
        .evaluate_module_entry("entry.js", &mut resolve_empty_host)
        .expect_err("empty canonical key should violate host contract");
    assert_eq!(
        contract_err,
        VmError::TypeError("ModuleLifecycle:HostContractViolation")
    );

    let mut load_fail_host = MemoryModuleHost::default().with_failing_load("entry.js");
    let mut vm = Vm::default();
    let load_err = vm
        .evaluate_module_entry("entry.js", &mut load_fail_host)
        .expect_err("load failure should map to deterministic type error");
    assert_eq!(load_err, VmError::TypeError("ModuleLifecycle:LoadFailed"));
}

#[test]
fn module_graph_instantiate_evaluate() {
    let mut host = MemoryModuleHost::default()
        .with_module(
            "entry.js",
            "import { inc } from './mid.js';\nexport const answer = inc + 1;\n",
        )
        .with_module(
            "mid.js",
            "import { base } from './dep.js';\nexport const inc = base + 1;\n",
        )
        .with_module("dep.js", "export const base = 40;\n");
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("module graph evaluation should succeed");

    assert_eq!(load_number_export(&exports, "answer"), 42.0);
    assert_eq!(
        vm.module_evaluation_order(),
        vec![
            "dep.js".to_string(),
            "mid.js".to_string(),
            "entry.js".to_string()
        ]
    );
    assert_eq!(
        vm.module_state("dep.js"),
        Some(ModuleLifecycleState::Evaluated)
    );
    assert_eq!(
        vm.module_state("mid.js"),
        Some(ModuleLifecycleState::Evaluated)
    );
    assert_eq!(
        vm.module_state("entry.js"),
        Some(ModuleLifecycleState::Evaluated)
    );
}

#[test]
fn module_promise_builtin_parity() {
    let mut host = MemoryModuleHost::default().with_module(
        "entry.js",
        "const direct = Promise;\n\
         const seed = new Promise(function (resolve) { resolve(20); });\n\
         const chained = seed.then(function (value) { return value + 22; });\n\
         export const promise_type = typeof direct;\n\
         export const chained_type = typeof chained;\n\
         export const has_then = typeof chained.then;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("module Promise usage should evaluate successfully");
    assert_eq!(load_string_export(&exports, "promise_type"), "function");
    assert_eq!(load_string_export(&exports, "chained_type"), "object");
    assert_eq!(load_string_export(&exports, "has_then"), "function");
}

#[test]
fn module_promise_queue_semantics() {
    let mut vm = evaluate_module_with_nested_promise_chain();
    let realm = Realm::default();
    let mut hooks = RecordingHooks::default();

    assert_eq!(vm.pending_promise_job_count(), 1);
    let first = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect("first drain should enqueue follow-up reaction");
    assert_eq!(
        first,
        PromiseJobDrainReport {
            processed: 1,
            remaining: 1,
            stop_reason: PromiseJobDrainStopReason::BudgetExhausted,
        }
    );
    assert_eq!(vm.pending_promise_job_count(), 1);

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
    assert_eq!(vm.pending_promise_job_count(), 0);
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
fn module_host_hook_drain_through_module_jobs() {
    let realm = Realm::default();

    let mut vm = evaluate_module_with_nested_promise_chain();
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
    assert_eq!(vm.pending_promise_job_count(), 1);

    let mut vm = evaluate_module_with_nested_promise_chain();
    let mut hooks = RecordingHooks {
        fail_on_enqueue: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("nested enqueue callback failure should be typed");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnEnqueueFailed")
    );

    let mut vm = evaluate_module_with_nested_promise_chain();
    let mut hooks = RecordingHooks {
        fail_on_drain_end: true,
        ..RecordingHooks::default()
    };
    let err = vm
        .drain_promise_jobs_with_host_hooks(1, &realm, false, &mut hooks)
        .expect_err("drain-end callback failure should be typed");
    assert_eq!(
        err,
        VmError::TypeError("PromiseJobQueue:HostOnDrainEndFailed")
    );
}

#[test]
fn module_cycle_and_failure_replay() {
    let mut cycle_host = MemoryModuleHost::default()
        .with_module("a.js", "import { b } from './b.js';\nexport const a = 1;\n")
        .with_module("b.js", "import { a } from './a.js';\nexport const b = 2;\n");
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("a.js", &mut cycle_host)
        .expect("cycle should evaluate deterministically");
    assert_eq!(load_number_export(&exports, "a"), 1.0);
    assert_eq!(
        vm.module_evaluation_order(),
        vec!["b.js".to_string(), "a.js".to_string()]
    );

    let mut failure_host =
        MemoryModuleHost::default().with_module("bad.js", "export const broken = ;\n");
    let mut vm = Vm::default();
    let first_err = vm
        .evaluate_module_entry("bad.js", &mut failure_host)
        .expect_err("invalid module source should fail during parse");
    let second_err = vm
        .evaluate_module_entry("bad.js", &mut failure_host)
        .expect_err("failed record should replay deterministically");
    assert_eq!(first_err, VmError::TypeError("ModuleLifecycle:ParseFailed"));
    assert_eq!(
        second_err,
        VmError::TypeError("ModuleLifecycle:ParseFailed")
    );
    assert_eq!(failure_host.load_count("bad.js"), 1);
}

#[test]
fn module_namespace_import_replay_determinism() {
    let mut host = MemoryModuleHost::default()
        .with_module("ns.js", "export const value = 1;\nexport default 7;\n")
        .with_module(
            "entry.js",
            "import * as ns from './ns.js';\nexport const value = ns.value;\nexport const defaultValue = ns.default;\n",
        );
    let mut vm = Vm::default();
    let first = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("namespace import should evaluate");
    let second = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("cached namespace import should replay deterministically");
    assert_eq!(load_number_export(&first, "value"), 1.0);
    assert_eq!(load_number_export(&first, "defaultValue"), 7.0);
    assert_eq!(load_number_export(&second, "value"), 1.0);
    assert_eq!(load_number_export(&second, "defaultValue"), 7.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("ns.js"), 1);
}

#[test]
fn module_named_reexport_replay_determinism() {
    let mut host = MemoryModuleHost::default()
        .with_module("dep.js", "export const value = 42;\nexport default 7;\n")
        .with_module(
            "entry.js",
            "export { value as answer, default as fallback } from './dep.js';\n",
        );
    let mut vm = Vm::default();
    let first = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("named re-export should evaluate");
    let second = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("cached named re-export should replay deterministically");
    assert_eq!(load_number_export(&first, "answer"), 42.0);
    assert_eq!(load_number_export(&first, "fallback"), 7.0);
    assert_eq!(load_number_export(&second, "answer"), 42.0);
    assert_eq!(load_number_export(&second, "fallback"), 7.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn module_export_star_replay_determinism() {
    let mut host = MemoryModuleHost::default()
        .with_module("dep.js", "export const value = 42;\nexport default 7;\n")
        .with_module(
            "bridge.js",
            "export * from './dep.js';\nexport const bridge = 1;\n",
        )
        .with_module(
            "entry.js",
            "import { value, bridge, default as fallback } from './bridge.js';\n\
             export const answer = value + bridge;\n\
             export const fallbackType = typeof fallback;\n",
        );
    let mut vm = Vm::default();
    let first = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("export-star re-export should evaluate");
    let second = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("cached export-star re-export should replay deterministically");
    assert_eq!(load_number_export(&first, "answer"), 43.0);
    assert_eq!(load_number_export(&second, "answer"), 43.0);
    assert_eq!(load_string_export(&first, "fallbackType"), "undefined");
    assert_eq!(load_string_export(&second, "fallbackType"), "undefined");
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn module_export_star_namespace_replay_determinism() {
    let mut host = MemoryModuleHost::default()
        .with_module("dep.js", "export const value = 42;\nexport default 7;\n")
        .with_module("bridge.js", "export * as ns from './dep.js';\n")
        .with_module(
            "entry.js",
            "import { ns } from './bridge.js';\n\
             export const answer = ns.value + ns.default;\n\
             export const nsType = typeof ns;\n",
        );
    let mut vm = Vm::default();
    let first = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("export-star namespace re-export should evaluate");
    let second = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("cached export-star namespace re-export should replay deterministically");
    assert_eq!(load_number_export(&first, "answer"), 49.0);
    assert_eq!(load_number_export(&second, "answer"), 49.0);
    assert_eq!(load_string_export(&first, "nsType"), "object");
    assert_eq!(load_string_export(&second, "nsType"), "object");
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn module_empty_named_import_keeps_dependency_edge() {
    let mut host = MemoryModuleHost::default()
        .with_module("dep.js", "export const value = 1;\n")
        .with_module(
            "entry.js",
            "import {} from './dep.js';\nexport const answer = 42;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("empty named import should still evaluate dependency");
    assert_eq!(load_number_export(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
    assert_eq!(vm.module_evaluation_count("entry.js"), Some(1));
    assert_eq!(vm.module_evaluation_count("dep.js"), Some(1));
}

#[test]
fn module_import_with_extra_from_spacing_parses_and_evaluates() {
    let mut host = MemoryModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import { value }   from   './dep.js';\nexport const answer = value + 1;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("import with extra from spacing should evaluate");
    assert_eq!(load_number_export(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn module_semicolonless_import_export_parses_and_evaluates() {
    let mut host = MemoryModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import { value } from './dep.js'\nexport const answer = value + 1\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("semicolonless module import/export should evaluate");
    assert_eq!(load_number_export(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn module_cache_gc_root_integrity() {
    let mut host =
        MemoryModuleHost::default().with_module("entry.js", "export const answer = 42;\n");
    let mut vm = Vm::default();
    let _ = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("module evaluation should succeed");
    assert_eq!(vm.module_cache_len(), 1);

    let stats_before_clear = vm.collect_garbage(&Realm::default());
    vm.clear_module_cache();
    let stats_after_clear = vm.collect_garbage(&Realm::default());
    assert_eq!(vm.module_cache_len(), 0);
    assert!(
        stats_after_clear.reclaimed_objects > stats_before_clear.reclaimed_objects,
        "reclaimed object count should increase after releasing cached module roots",
    );
}
