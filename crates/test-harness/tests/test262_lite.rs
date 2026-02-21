#![forbid(unsafe_code)]

use std::path::PathBuf;
use test_harness::test262::{SuiteOptions, run_suite};

#[test]
fn runs_test262_lite_suite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("test262-lite");
    let summary = run_suite(&root, SuiteOptions::default()).expect("suite execution should pass");
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
