#![forbid(unsafe_code)]

use runtime::{JsValue, ModuleLifecycleState, Realm};
use std::collections::{BTreeMap, BTreeSet};
use vm::{ModuleHost, ModuleHostError, Vm, VmError};

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
fn module_error_replay_determinism() {
    let mut host = MemoryModuleHost::default()
        .with_module("ns.js", "export const value = 1;\n")
        .with_module(
            "entry.js",
            "import * as ns from './ns.js';\nexport const value = 1;\n",
        );
    let mut vm = Vm::default();
    let first_err = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect_err("unsupported namespace import execution path should fail");
    let second_err = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect_err("failed module should replay deterministic error");
    assert_eq!(
        first_err,
        VmError::TypeError("ModuleLifecycle:EvaluateFailed")
    );
    assert_eq!(
        second_err,
        VmError::TypeError("ModuleLifecycle:EvaluateFailed")
    );
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("ns.js"), 1);
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
