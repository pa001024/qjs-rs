#![forbid(unsafe_code)]

use std::path::PathBuf;
use test_harness::test262::{SuiteOptions, run_suite};

#[test]
fn runs_test262_lite_suite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("test262-lite");
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
    assert_eq!(
        summary.failed, 0,
        "test262-lite should have zero mismatches"
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
