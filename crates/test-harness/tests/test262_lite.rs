#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use test_harness::run_script;

#[test]
fn runs_test262_lite_suite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("test262-lite");

    assert_category(&root.join("pass"), Expectation::Pass);
    assert_category(&root.join("fail").join("parse"), Expectation::Fail);
    assert_category(&root.join("fail").join("runtime"), Expectation::Fail);
}

#[derive(Clone, Copy, Debug)]
enum Expectation {
    Pass,
    Fail,
}

fn assert_category(root: &Path, expectation: Expectation) {
    let files = collect_js_files(root);
    assert!(
        !files.is_empty(),
        "no test262-lite cases found in {}",
        root.display()
    );

    for file in files {
        let source = fs::read_to_string(&file).unwrap_or_else(|err| {
            panic!("failed to read {}: {err}", file.display());
        });
        let result = run_script(&source, &[]);
        match expectation {
            Expectation::Pass => {
                assert!(
                    result.is_ok(),
                    "expected pass but failed: {} => {:?}",
                    file.display(),
                    result
                );
            }
            Expectation::Fail => {
                assert!(
                    result.is_err(),
                    "expected fail but passed: {} => {:?}",
                    file.display(),
                    result
                );
            }
        }
    }
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
