#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use test_harness::test262::{
    ExecutionOutcome, ExpectedOutcome, execute_case, expected_outcome, parse_test262_case,
    should_skip,
};

#[test]
fn runs_test262_lite_suite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("test262-lite");

    let files = collect_js_files(&root);
    assert!(
        !files.is_empty(),
        "no test262-lite cases found in {}",
        root.display()
    );

    let mut executed = 0usize;
    let mut skipped = 0usize;
    for file in files {
        let source = fs::read_to_string(&file).unwrap_or_else(|err| {
            panic!("failed to read {}: {err}", file.display());
        });
        let case = parse_test262_case(&source).unwrap_or_else(|err| {
            panic!("failed to parse frontmatter in {}: {err}", file.display());
        });

        if should_skip(&case.frontmatter) {
            skipped += 1;
            continue;
        }
        executed += 1;

        let expected = expected_outcome(&case.frontmatter);
        let actual = execute_case(case.body);
        match (expected, actual) {
            (ExpectedOutcome::Pass, ExecutionOutcome::Pass)
            | (ExpectedOutcome::ParseFail, ExecutionOutcome::ParseFail(_))
            | (ExpectedOutcome::RuntimeFail, ExecutionOutcome::RuntimeFail(_)) => {}
            (expected, actual) => {
                panic!(
                    "unexpected result for {}: expected {:?}, got {:?}",
                    file.display(),
                    expected,
                    actual
                );
            }
        }
    }

    assert!(executed > 0, "all test262-lite cases were skipped");
    assert!(
        skipped < executed + skipped,
        "skipped count invariant should hold"
    );
}

fn collect_js_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit_dir(root, &mut files);
    files.sort();
    files
}

fn visit_dir(root: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root).unwrap_or_else(|err| {
        panic!("failed to read dir {}: {err}", root.display());
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!(
                "failed to read directory entry in {}: {err}",
                root.display()
            );
        });
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, files);
            continue;
        }
        if matches!(path.extension().and_then(|ext| ext.to_str()), Some("js")) {
            files.push(path);
        }
    }
}
