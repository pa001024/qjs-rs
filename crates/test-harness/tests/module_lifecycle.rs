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

#[test]
fn export_star_paths_are_deterministic() {
    let mut host = HarnessModuleHost::default()
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
    assert_eq!(expect_number(&first, "answer"), 43.0);
    assert_eq!(expect_number(&second, "answer"), 43.0);
    assert_eq!(expect_string(&first, "fallbackType"), "undefined");
    assert_eq!(expect_string(&second, "fallbackType"), "undefined");
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn export_star_namespace_paths_are_deterministic() {
    let mut host = HarnessModuleHost::default()
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
    assert_eq!(expect_number(&first, "answer"), 49.0);
    assert_eq!(expect_number(&second, "answer"), 49.0);
    assert_eq!(expect_string(&first, "nsType"), "object");
    assert_eq!(expect_string(&second, "nsType"), "object");
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn empty_named_import_keeps_dependency_edge() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 1;\n")
        .with_module(
            "entry.js",
            "import {} from './dep.js';\nexport const answer = 42;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("empty named import should still evaluate dependency");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
    assert_eq!(vm.module_evaluation_count("entry.js"), Some(1));
    assert_eq!(vm.module_evaluation_count("dep.js"), Some(1));
}

#[test]
fn import_with_extra_from_spacing_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import { value }   from   './dep.js';\nexport const answer = value + 1;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("import with extra from spacing should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn semicolonless_import_export_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import { value } from './dep.js'\nexport const answer = value + 1\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("semicolonless module import/export should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn compact_keyword_spacing_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import{ value }from'./dep.js'\nconst answer = value + 1\nexport{answer}\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("compact module keyword spacing should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn trailing_line_comment_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("from-token-dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import { value } from './from-token-dep.js' // from trailing comment\n\
             export const answer = value + 1 // semicolonless with comment\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("trailing comment module declarations should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("from-token-dep.js"), 1);
}

#[test]
fn compact_reexport_from_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "bridge.js",
            "export*from'./dep.js'\nexport{value as answer}from'./dep.js'\n",
        )
        .with_module(
            "entry.js",
            "import { value, answer } from './bridge.js';\nexport const total = value + answer;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("compact re-export from syntax should evaluate");
    assert_eq!(expect_number(&exports, "total"), 82.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn multiline_import_export_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 40;\nexport const extra = 2;\n")
        .with_module(
            "entry.js",
            "import {\n  value,\n  extra as bonus,\n}\nfrom\n  './dep.js'\nexport const answer =\n  value + bonus\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("multiline module import/export should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn multiline_named_reexport_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 42;\nexport default 7;\n")
        .with_module(
            "bridge.js",
            "export {\n  value as answer,\n  default as fallback,\n}\nfrom\n  './dep.js'\n",
        )
        .with_module(
            "entry.js",
            "import { answer, fallback } from './bridge.js';\nexport const total = answer + fallback;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("multiline named re-export should evaluate");
    assert_eq!(expect_number(&exports, "total"), 49.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn destructuring_export_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default().with_module(
        "entry.js",
        "const payload = { value: 40, extra: 2 };\n\
         export const { value, extra } = payload;\n\
         export const [first, , third] = [1, 2, 3];\n\
         export const left = { a: 1, b: 2 }, right = 40 + 2;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("destructuring module export declarations should evaluate");
    assert_eq!(expect_number(&exports, "value"), 40.0);
    assert_eq!(expect_number(&exports, "extra"), 2.0);
    assert_eq!(expect_number(&exports, "first"), 1.0);
    assert_eq!(expect_number(&exports, "third"), 3.0);
    assert_eq!(expect_number(&exports, "right"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
}

#[test]
fn keyword_identifier_names_in_clauses_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "const value = 42;\nexport { value as if };\n")
        .with_module(
            "entry.js",
            "import { if as condition } from './dep.js';\nexport { condition as while };\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("keyword identifier names in module clauses should evaluate");
    assert_eq!(expect_number(&exports, "while"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn generator_export_declaration_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default().with_module(
        "entry.js",
        "export function* values() { yield 40; yield 2; }\n\
         const iter = values();\n\
         export const total = iter.next().value + iter.next().value;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("generator export declaration should evaluate");
    assert_eq!(expect_number(&exports, "total"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
}

#[test]
fn string_named_import_export_clauses_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module(
            "dep.js",
            "const value = 42;\nexport { value as \"kebab-name\" };\n",
        )
        .with_module(
            "entry.js",
            "import { \"kebab-name\" as kebabName } from './dep.js';\nexport const answer = kebabName;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("string-named import/export clauses should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn multiline_default_export_expression_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export default\n  42\n")
        .with_module(
            "entry.js",
            "import value from './dep.js';\nexport const answer = value;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("multiline default export expression should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn default_named_function_declaration_binding_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default().with_module(
        "entry.js",
        "export default function Named() { return 41; }\n\
         export const answer = Named() + 1;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("default named function declaration export should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
}

#[test]
fn default_named_class_declaration_binding_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default().with_module(
        "entry.js",
        "export default class Counter { static base() { return 41; } }\n\
         export const answer = Counter.base() + 1;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("default named class declaration export should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
}

#[test]
fn import_with_attributes_clause_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import { value } from './dep.js' with { type: 'json' };\nexport const answer = value + 1;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("import with attributes clause should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn reexport_with_attributes_clause_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 42;\n")
        .with_module(
            "bridge.js",
            "export { value as answer } from './dep.js' assert { type: 'json' };\n",
        )
        .with_module(
            "entry.js",
            "import { answer } from './bridge.js';\nexport const total = answer;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("re-export with attributes clause should evaluate");
    assert_eq!(expect_number(&exports, "total"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn multiline_import_with_attributes_clause_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 41;\n")
        .with_module(
            "entry.js",
            "import { value } from './dep.js'\nwith { type: 'json' }\nexport const answer = value + 1;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("multiline import attributes clause should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn multiline_reexport_with_attributes_clause_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 42;\n")
        .with_module(
            "bridge.js",
            "export { value as answer } from './dep.js'\nassert { type: 'json' }\n",
        )
        .with_module(
            "entry.js",
            "import { answer } from './bridge.js';\nexport const total = answer;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("multiline re-export attributes clause should evaluate");
    assert_eq!(expect_number(&exports, "total"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn default_named_generator_declaration_binding_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default().with_module(
        "entry.js",
        "export default function* Gen() { yield 40; yield 2; }\n\
         const iter = Gen();\n\
         export const total = iter.next().value + iter.next().value;\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("default named generator declaration export should evaluate");
    assert_eq!(expect_number(&exports, "total"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
}

#[test]
fn string_named_reexport_clause_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 42;\n")
        .with_module(
            "bridge.js",
            "export { value as \"kebab-name\" } from './dep.js';\n",
        )
        .with_module(
            "entry.js",
            "import { \"kebab-name\" as kebabName } from './bridge.js';\nexport const answer = kebabName;\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("string-named re-export clause should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("bridge.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}

#[test]
fn multiline_export_function_declaration_body_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default().with_module(
        "entry.js",
        "export function build()\n{\n  return 42;\n}\nexport const answer = build();\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("multiline export function declaration body should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
}

#[test]
fn multiline_default_class_declaration_body_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default().with_module(
        "entry.js",
        "export default class Counter\n{\n  static value() { return 42; }\n}\nexport const answer = Counter.value();\n",
    );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("multiline default class declaration body should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
}

#[test]
fn linebreak_as_alias_clauses_parses_and_evaluates() {
    let mut host = HarnessModuleHost::default()
        .with_module("dep.js", "export const value = 42;\n")
        .with_module(
            "entry.js",
            "import {\n  value\n  as\n  alias,\n} from './dep.js';\nexport {\n  alias\n  as\n  answer,\n};\n",
        );
    let mut vm = Vm::default();
    let exports = vm
        .evaluate_module_entry("entry.js", &mut host)
        .expect("linebreak as-alias clauses should evaluate");
    assert_eq!(expect_number(&exports, "answer"), 42.0);
    assert_eq!(host.load_count("entry.js"), 1);
    assert_eq!(host.load_count("dep.js"), 1);
}
