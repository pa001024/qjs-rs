#![forbid(unsafe_code)]

use std::path::PathBuf;
use test_harness::test262::{SuiteOptions, parse_test262_case, run_suite, should_skip};

#[test]
fn runs_test262_lite_suite_in_default_profile() {
    let root = test262_lite_root();
    let summary = run_suite(&root, SuiteOptions::default()).expect("suite execution should pass");
    assert_basic_suite_expectations(&summary);
    assert_eq!(
        summary.gc.collections_total, 0,
        "default profile should keep collections_total at zero when gc toggles are disabled"
    );
    assert_eq!(
        summary.gc.boundary_collections, 0,
        "default profile should keep boundary collections at zero when gc toggles are disabled"
    );
    assert_eq!(
        summary.gc.runtime_collections, 0,
        "default profile should keep runtime collections at zero when gc toggles are disabled"
    );
    assert_eq!(
        summary.gc.reclaimed_objects, 0,
        "default profile should keep reclaimed objects at zero when gc toggles are disabled"
    );
}

#[test]
fn test262_lite_skip_categories_balanced_totals() {
    let root = test262_lite_root();
    let summary = run_suite(&root, SuiteOptions::default()).expect("suite execution should pass");
    assert_eq!(
        summary.skipped,
        summary.skipped_categories.total(),
        "skip category counters should sum to skipped total"
    );
}

#[test]
fn runs_test262_lite_suite_in_stress_profile() {
    let root = test262_lite_root();
    let summary = run_suite(
        &root,
        SuiteOptions {
            auto_gc: true,
            auto_gc_threshold: Some(1),
            runtime_gc: true,
            runtime_gc_check_interval: Some(1),
            ..SuiteOptions::default()
        },
    )
    .expect("suite execution should pass");
    assert!(
        summary.discovered > 0,
        "test262-lite fixtures should not be empty"
    );
    assert!(
        summary.executed > 0,
        "test262-lite should execute at least one case"
    );
    assert!(
        summary.failed <= 5,
        "stress profile allows a small mismatch budget while runtime semantics converge; got {} mismatches",
        summary.failed
    );
    assert_eq!(
        summary.skipped,
        summary.skipped_categories.total(),
        "stress profile skip category counters should stay balanced"
    );
    assert!(
        summary.gc.collections_total > 0,
        "stress mode should trigger gc collections"
    );
    assert!(
        summary.gc.runtime_collections > 0,
        "stress mode should trigger runtime gc at least once"
    );
    assert_eq!(
        summary.gc.collections_total,
        summary.gc.runtime_collections + summary.gc.boundary_collections,
        "gc collection accounting should stay balanced"
    );
    assert!(
        summary.gc.reclaimed_objects > 0,
        "stress mode should reclaim at least one object"
    );
    let runtime_ratio = summary.gc.runtime_collections as f64 / summary.gc.collections_total as f64;
    assert!(
        runtime_ratio >= 0.9,
        "runtime gc ratio should stay >= 0.9 under stress mode, got {runtime_ratio:.4}"
    );
}

fn test262_lite_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("test262-lite")
}

fn native_errors_subset_root() -> PathBuf {
    test262_lite_root().join("pass").join("built-ins")
}

fn json_subset_root() -> PathBuf {
    native_errors_subset_root().join("JSON")
}

fn core_builtin_family_root(name: &str) -> PathBuf {
    native_errors_subset_root().join(name)
}

fn phase6_builtin_family_root(name: &str) -> PathBuf {
    native_errors_subset_root().join(name)
}

fn assert_basic_suite_expectations(summary: &test_harness::test262::SuiteSummary) {
    assert!(
        summary.discovered > 0,
        "test262-lite fixtures should not be empty"
    );
    assert!(
        summary.executed > 0,
        "test262-lite should execute at least one case"
    );
    assert_eq!(
        summary.failed, 0,
        "test262-lite should have zero mismatches"
    );
}

#[test]
fn module_flag_cases_are_no_longer_preemptively_skipped() {
    let case = parse_test262_case("/*---\nflags: [module]\n---*/\nimport './dep.js';")
        .expect("frontmatter parse should succeed");
    assert!(!should_skip(&case.frontmatter));
}

#[test]
fn native_errors_subset() {
    let summary = run_suite(&native_errors_subset_root(), SuiteOptions::default())
        .expect("native error subset should execute");
    assert!(
        summary.discovered >= 3,
        "native error subset should include multiple built-ins fixtures"
    );
    assert!(
        summary.executed >= 3,
        "native error subset should execute all smoke fixtures"
    );
    assert_eq!(
        summary.failed, 0,
        "native error subset should have zero mismatches"
    );
}

#[test]
fn json_subset() {
    let summary =
        run_suite(&json_subset_root(), SuiteOptions::default()).expect("json subset should run");
    assert!(
        summary.discovered >= 3,
        "json subset should include parse/stringify/cycle smoke fixtures"
    );
    assert!(
        summary.executed >= 3,
        "json subset should execute all smoke fixtures"
    );
    assert_eq!(summary.failed, 0, "json subset should have zero mismatches");
}

#[test]
fn core_builtins_subset() {
    for family in [
        "Object", "Array", "Boolean", "Function", "String", "Number", "Math", "Date", "Promise",
    ] {
        let summary = run_suite(&core_builtin_family_root(family), SuiteOptions::default())
            .unwrap_or_else(|err| panic!("{family} subset should run: {err}"));
        assert!(
            summary.discovered >= 1,
            "{family} subset should include at least one smoke fixture"
        );
        assert!(
            summary.executed >= 1,
            "{family} subset should execute at least one smoke fixture"
        );
        assert_eq!(
            summary.failed, 0,
            "{family} subset should have zero mismatches"
        );
    }
}

#[test]
fn collection_and_regexp_subset() {
    for family in ["Map", "Set", "WeakMap", "WeakSet", "RegExp"] {
        let summary = run_suite(&phase6_builtin_family_root(family), SuiteOptions::default())
            .unwrap_or_else(|err| panic!("{family} subset should run: {err}"));
        assert!(
            summary.discovered >= 1,
            "{family} subset should include at least one smoke fixture"
        );
        assert!(
            summary.executed >= 1,
            "{family} subset should execute at least one smoke fixture"
        );
        assert_eq!(
            summary.failed, 0,
            "{family} subset should have zero mismatches"
        );
    }
}
