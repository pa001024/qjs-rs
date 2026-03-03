#![forbid(unsafe_code)]

use runtime::JsValue;
use std::collections::{BTreeMap, BTreeSet};
use test_harness::run_module_entry;
use vm::{ModuleHost, ModuleHostError, Vm, VmError};

#[derive(Debug, Default)]
struct HarnessModuleHost {
    modules: BTreeMap<String, String>,
    load_counts: BTreeMap<String, usize>,
    fail_load_keys: BTreeSet<String>,
}

impl HarnessModuleHost {
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

impl ModuleHost for HarnessModuleHost {
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

fn expect_number(exports: &BTreeMap<String, JsValue>, key: &str) -> f64 {
    match exports.get(key).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Number(number) => number,
        other => panic!("expected numeric export {key}, got {other:?}"),
    }
}

fn expect_string(exports: &BTreeMap<String, JsValue>, key: &str) -> String {
    match exports.get(key).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::String(text) => text,
        other => panic!("expected string export {key}, got {other:?}"),
    }
}

#[test]
fn evaluates_static_module_graph_baseline() {
    let exports = run_module_entry(
        "entry.js",
        &[
            ("dep.js", "export const inc = 41;\n"),
            (
                "entry.js",
                "import { inc } from './dep.js';\nexport const answer = inc + 1;\n",
            ),
        ],
    )
    .expect("module graph should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
}

#[test]
fn module_entry_promise_builtin_parity() {
    let exports = run_module_entry(
        "entry.js",
        &[(
            "entry.js",
            "const direct = Promise;\n\
             const seed = new Promise(function (resolve) { resolve(20); });\n\
             const chained = seed.then(function (value) { return value + 22; });\n\
             export const promise_type = typeof direct;\n\
             export const chained_type = typeof chained;\n\
             export const has_then = typeof chained.then;\n",
        )],
    )
    .expect("module entry Promise usage should evaluate");
    assert_eq!(expect_string(&exports, "promise_type"), "function");
    assert_eq!(expect_string(&exports, "chained_type"), "object");
    assert_eq!(expect_string(&exports, "has_then"), "function");
}

#[test]
fn reimport_reuses_cache_without_duplicate_execution() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const inc = 41;\n")
        .with_module(
            "entry.js",
            "import { inc } from './dep.js';\n\
             const seed = new Promise(function (resolve) { resolve(inc); });\n\
             const chained = seed.then(function (value) { return value + 1; });\n\
             export const answer = inc + 1;\n\
             export const promise_type = typeof Promise;\n\
             export const chained_type = typeof chained;\n",
        );
    let mut vm = Vm::default();
    let first = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("first import should succeed");
    let second = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("second import should reuse cache");
    assert_eq!(expect_number(&first, "answer"), 42.0);
    assert_eq!(expect_number(&second, "answer"), 42.0);
    assert_eq!(expect_string(&first, "promise_type"), "function");
    assert_eq!(expect_string(&first, "chained_type"), "object");
    assert_eq!(expect_string(&second, "promise_type"), "function");
    assert_eq!(expect_string(&second, "chained_type"), "object");
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
    assert_eq!(vm.module_evaluation_count("entry.js"), Some(1));
    assert_eq!(vm.module_evaluation_count("dep.js"), Some(1));
}

#[test]
fn cycle_and_failure_paths_are_deterministic() {
    let mut cycle_host = HarnessModuleHost::default()
        .with_module("a.js", "import { b } from './b.js';\nexport const a = 1;\n")
        .with_module("b.js", "import { a } from './a.js';\nexport const b = 2;\n");
    let mut vm = Vm::default();
    let cycle_exports = vm
        .evaluate_module_entry("a.js", &mut cycle_host)
        .expect("cycle should evaluate");
    assert_eq!(expect_number(&cycle_exports, "a"), 1.0);
    assert_eq!(
        vm.module_evaluation_order(),
        vec!["b.js".to_string(), "a.js".to_string()]
    );

    let mut failure_host = HarnessModuleHost::default()
        .with_module("entry.js", "export const value = 1;\n")
        .with_failing_load("entry.js");
    let mut vm = Vm::default();
    let first_err = vm
        .evaluate_module_entry("entry.js", &mut failure_host)
        .expect_err("load failure should fail deterministically");
    let second_err = vm
        .evaluate_module_entry("entry.js", &mut failure_host)
        .expect_err("reimport should keep deterministic failure category");
    assert_eq!(first_err, VmError::TypeError("ModuleLifecycle:LoadFailed"));
    assert_eq!(second_err, VmError::TypeError("ModuleLifecycle:LoadFailed"));

    let mut namespace_host = HarnessModuleHost::default()
        .with_module("ns.js", "export const value = 1;\nexport default 7;\n")
        .with_module(
            "entry.js",
            "import * as ns from './ns.js';\nexport const value = ns.value;\nexport const defaultValue = ns.default;\n",
        );
    let mut vm = Vm::default();
    let first_namespace = vm
        .evaluate_module_entry("entry.js", &mut namespace_host)
        .expect("namespace import should evaluate");
    let second_namespace = vm
        .evaluate_module_entry("entry.js", &mut namespace_host)
        .expect("cached namespace import should replay deterministically");
    assert_eq!(expect_number(&first_namespace, "value"), 1.0);
    assert_eq!(expect_number(&first_namespace, "defaultValue"), 7.0);
    assert_eq!(expect_number(&second_namespace, "value"), 1.0);
    assert_eq!(expect_number(&second_namespace, "defaultValue"), 7.0);
    assert_eq!(namespace_host.load_count("entry.js"), 1);
    assert_eq!(namespace_host.load_count("ns.js"), 1);
}

#[test]
fn named_reexport_paths_are_deterministic() {
    let mut host = HarnessModuleHost::default()
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
    assert_eq!(expect_number(&first, "answer"), 42.0);
    assert_eq!(expect_number(&first, "fallback"), 7.0);
    assert_eq!(expect_number(&second, "answer"), 42.0);
    assert_eq!(expect_number(&second, "fallback"), 7.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}
