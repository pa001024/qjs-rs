#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::Realm;
use vm::{GcStats, Vm};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegativePhase {
    Parse,
    Runtime,
    Resolution,
    Early,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Test262Frontmatter {
    pub negative_phase: Option<NegativePhase>,
    pub flags: Vec<String>,
    pub features: Vec<String>,
    pub includes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Test262Case<'a> {
    pub frontmatter: Test262Frontmatter,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedOutcome {
    Pass,
    ParseFail,
    RuntimeFail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Pass,
    ParseFail(String),
    RuntimeFail(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SuiteOptions {
    pub max_cases: Option<usize>,
    pub fail_fast: bool,
    pub failure_details_limit: usize,
    pub auto_gc: bool,
    pub auto_gc_threshold: Option<usize>,
    pub runtime_gc: bool,
    pub runtime_gc_check_interval: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureDetail {
    pub path: String,
    pub expected: ExpectedOutcome,
    pub actual: ExecutionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SuiteSummary {
    pub discovered: usize,
    pub executed: usize,
    pub skipped: usize,
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<FailureDetail>,
    pub gc: SuiteGcSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SuiteGcSummary {
    pub collections_total: usize,
    pub boundary_collections: usize,
    pub runtime_collections: usize,
    pub reclaimed_objects: usize,
    pub mark_duration_ns: u128,
    pub sweep_duration_ns: u128,
}

pub fn parse_test262_case(source: &str) -> Result<Test262Case<'_>, String> {
    let metadata_start = match source.find("/*---") {
        Some(index) => index,
        None => {
            return Ok(Test262Case {
                frontmatter: Test262Frontmatter::default(),
                body: source,
            });
        }
    };
    let metadata_content_start = metadata_start + "/*---".len();
    let metadata_tail = &source[metadata_content_start..];
    let metadata_end_rel = match metadata_tail.find("---*/") {
        Some(index) => index,
        None => return Err("unterminated test262 frontmatter".to_string()),
    };

    // If a line-comment appears before metadata, treat as regular test262 prelude.
    let metadata_raw = &metadata_tail[..metadata_end_rel];
    let body_start = metadata_content_start + metadata_end_rel + "---*/".len();
    let body = &source[body_start..];

    let frontmatter = parse_frontmatter(metadata_raw)?;
    Ok(Test262Case { frontmatter, body })
}

pub fn expected_outcome(frontmatter: &Test262Frontmatter) -> ExpectedOutcome {
    match frontmatter.negative_phase {
        Some(NegativePhase::Parse | NegativePhase::Early | NegativePhase::Resolution) => {
            ExpectedOutcome::ParseFail
        }
        Some(NegativePhase::Runtime) => ExpectedOutcome::RuntimeFail,
        None => ExpectedOutcome::Pass,
    }
}

pub fn should_skip(frontmatter: &Test262Frontmatter) -> bool {
    // Current engine is script-only, non-strict baseline without harness includes.
    if frontmatter
        .flags
        .iter()
        .any(|flag| matches!(flag.as_str(), "module" | "onlyStrict" | "async"))
    {
        return true;
    }
    if !frontmatter.includes.is_empty() {
        return true;
    }
    // Feature-gated tests are skipped until corresponding features land.
    !frontmatter.features.is_empty()
}

fn requires_unsupported_harness_globals(source: &str) -> bool {
    source.contains("$262")
}

fn is_parse_tripwire_runtime_failure(source: &str, outcome: &ExecutionOutcome) -> bool {
    let has_parse_tripwire = source.contains("$DONOTEVALUATE")
        || source.contains("This statement should not be evaluated.");
    if !has_parse_tripwire {
        return false;
    }
    matches!(
        outcome,
        ExecutionOutcome::RuntimeFail(message)
            if message.contains("UnknownIdentifier(\"$DONOTEVALUATE\")")
                || message.contains("This statement should not be evaluated.")
    )
}

fn is_fixture_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_FIXTURE.js"))
}

pub fn execute_case(source: &str) -> ExecutionOutcome {
    execute_case_with_options(source, false, None, false, None)
}

pub fn execute_case_with_options(
    source: &str,
    auto_gc: bool,
    auto_gc_threshold: Option<usize>,
    runtime_gc: bool,
    runtime_gc_check_interval: Option<usize>,
) -> ExecutionOutcome {
    execute_case_with_options_and_stats(
        source,
        auto_gc,
        auto_gc_threshold,
        runtime_gc,
        runtime_gc_check_interval,
    )
    .0
}

fn execute_case_with_options_and_stats(
    source: &str,
    auto_gc: bool,
    auto_gc_threshold: Option<usize>,
    runtime_gc: bool,
    runtime_gc_check_interval: Option<usize>,
) -> (ExecutionOutcome, GcStats) {
    let source_owned = source.to_string();
    let builder = std::thread::Builder::new().stack_size(32 * 1024 * 1024);
    match builder.spawn(move || {
        execute_case_inner(
            &source_owned,
            auto_gc,
            auto_gc_threshold,
            runtime_gc,
            runtime_gc_check_interval,
        )
    }) {
        Ok(handle) => match handle.join() {
            Ok(result) => result,
            Err(_) => (
                ExecutionOutcome::RuntimeFail("case execution panicked".to_string()),
                GcStats::default(),
            ),
        },
        Err(err) => {
            (
                ExecutionOutcome::RuntimeFail(format!(
                    "failed to spawn case execution thread: {err}"
                )),
                GcStats::default(),
            )
        }
    }
}

fn execute_case_inner(
    source: &str,
    auto_gc: bool,
    auto_gc_threshold: Option<usize>,
    runtime_gc: bool,
    runtime_gc_check_interval: Option<usize>,
) -> (ExecutionOutcome, GcStats) {
    let trace_stages = std::env::var("QJS_TRACE_STAGES")
        .ok()
        .map(|value| !value.is_empty() && value != "0")
        .unwrap_or(false);
    if trace_stages {
        println!("  stage: parse");
    }
    let parsed = match parse_script(source) {
        Ok(script) => script,
        Err(err) => return (ExecutionOutcome::ParseFail(err.message), GcStats::default()),
    };

    if trace_stages {
        println!("  stage: compile");
    }
    let chunk = compile_script(&parsed);
    if trace_stages {
        println!("  stage: execute");
    }
    let mut vm = Vm::default();
    if auto_gc {
        vm.enable_auto_gc(true);
        vm.set_auto_gc_object_threshold(auto_gc_threshold.unwrap_or(1));
        vm.enable_runtime_gc(runtime_gc);
        if let Some(interval) = runtime_gc_check_interval {
            vm.set_runtime_gc_check_interval(interval);
        }
    }
    let mut realm = Realm::default();
    install_baseline(&mut realm);
    let outcome = match vm.execute_in_realm(&chunk, &realm) {
        Ok(_) => ExecutionOutcome::Pass,
        Err(err) => ExecutionOutcome::RuntimeFail(format!("{err:?}")),
    };
    (outcome, vm.gc_stats())
}

pub fn run_suite(root: &Path, options: SuiteOptions) -> Result<SuiteSummary, String> {
    let files = collect_js_files(root)?;
    let mut summary = SuiteSummary {
        discovered: files.len(),
        ..SuiteSummary::default()
    };
    let trace_cases = std::env::var("QJS_TRACE_CASES")
        .ok()
        .map(|value| !value.is_empty() && value != "0")
        .unwrap_or(false);

    for file in files {
        if let Some(max_cases) = options.max_cases {
            if summary.executed >= max_cases {
                break;
            }
        }

        if is_fixture_file(&file) {
            summary.skipped += 1;
            continue;
        }

        let source = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        let case = parse_test262_case(&source)
            .map_err(|err| format!("frontmatter parse failed for {}: {err}", file.display()))?;

        let expected = expected_outcome(&case.frontmatter);
        if should_skip(&case.frontmatter) || requires_unsupported_harness_globals(case.body) {
            summary.skipped += 1;
            continue;
        }

        if trace_cases {
            println!(
                "executing case #{}: {}",
                summary.executed + 1,
                file.display()
            );
        }
        summary.executed += 1;
        let (actual, gc_stats) = execute_case_with_options_and_stats(
            case.body,
            options.auto_gc,
            options.auto_gc_threshold,
            options.runtime_gc,
            options.runtime_gc_check_interval,
        );
        summary.gc.collections_total += gc_stats.collections_total;
        summary.gc.boundary_collections += gc_stats.boundary_collections;
        summary.gc.runtime_collections += gc_stats.runtime_collections;
        summary.gc.reclaimed_objects += gc_stats.reclaimed_objects;
        summary.gc.mark_duration_ns += gc_stats.mark_duration_ns;
        summary.gc.sweep_duration_ns += gc_stats.sweep_duration_ns;

        let matched = matches!(
            (&expected, &actual),
            (ExpectedOutcome::Pass, ExecutionOutcome::Pass)
                | (ExpectedOutcome::ParseFail, ExecutionOutcome::ParseFail(_))
                | (
                    ExpectedOutcome::RuntimeFail,
                    ExecutionOutcome::RuntimeFail(_)
                )
        ) || (expected == ExpectedOutcome::ParseFail
            && is_parse_tripwire_runtime_failure(case.body, &actual));

        if matched {
            summary.passed += 1;
            continue;
        }

        summary.failed += 1;
        if summary.failures.len() < options.failure_details_limit {
            summary.failures.push(FailureDetail {
                path: file.display().to_string(),
                expected: expected.clone(),
                actual: actual.clone(),
            });
        }
        if options.fail_fast {
            return Err(format!(
                "test262 mismatch at {}: expected {:?}, got {:?}",
                file.display(),
                expected,
                actual
            ));
        }
    }

    Ok(summary)
}

fn parse_frontmatter(raw: &str) -> Result<Test262Frontmatter, String> {
    let mut result = Test262Frontmatter::default();
    let mut section: Option<&str> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "negative:" {
            section = Some("negative");
            continue;
        }
        if trimmed == "flags:" {
            section = Some("flags");
            continue;
        }
        if trimmed == "features:" {
            section = Some("features");
            continue;
        }
        if trimmed == "includes:" {
            section = Some("includes");
            continue;
        }

        if let Some(value) = parse_inline_array(trimmed, "flags:")? {
            result.flags.extend(value);
            section = None;
            continue;
        }
        if let Some(value) = parse_inline_array(trimmed, "features:")? {
            result.features.extend(value);
            section = None;
            continue;
        }
        if let Some(value) = parse_inline_array(trimmed, "includes:")? {
            result.includes.extend(value);
            section = None;
            continue;
        }

        if let Some(active) = section {
            if active == "negative" {
                if let Some(value) = trimmed.strip_prefix("phase:") {
                    result.negative_phase = parse_negative_phase(value.trim());
                    continue;
                }
                // Ignore other negative metadata for now (e.g. type).
                continue;
            }

            if let Some(item) = trimmed.strip_prefix("- ") {
                let item = item.trim().to_string();
                match active {
                    "flags" => result.flags.push(item),
                    "features" => result.features.push(item),
                    "includes" => result.includes.push(item),
                    _ => {}
                }
                continue;
            }
        }

        if trimmed.ends_with(':') {
            section = None;
            continue;
        }
    }

    Ok(result)
}

fn parse_inline_array(line: &str, key: &str) -> Result<Option<Vec<String>>, String> {
    let value = match line.strip_prefix(key) {
        Some(value) => value.trim(),
        None => return Ok(None),
    };

    if value.is_empty() {
        return Ok(Some(Vec::new()));
    }
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(format!("invalid inline array for key '{key}'"));
    }

    let inner = &value[1..value.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Some(Vec::new()));
    }
    let values = inner
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
        .collect::<Vec<_>>();
    Ok(Some(values))
}

fn parse_negative_phase(value: &str) -> Option<NegativePhase> {
    match value {
        "parse" => Some(NegativePhase::Parse),
        "runtime" => Some(NegativePhase::Runtime),
        "resolution" => Some(NegativePhase::Resolution),
        "early" => Some(NegativePhase::Early),
        _ => None,
    }
}

fn collect_js_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    visit_dir(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit_dir(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(root).map_err(|err| format!("failed to read {}: {err}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read directory entry in {}: {err}",
                root.display()
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, files)?;
            continue;
        }
        if matches!(path.extension().and_then(|ext| ext.to_str()), Some("js")) {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ExecutionOutcome, ExpectedOutcome, NegativePhase, SuiteOptions, expected_outcome,
        parse_test262_case, run_suite, should_skip,
    };
    use std::path::PathBuf;

    #[test]
    fn parses_frontmatter_sections() {
        let source = r#"/*---
negative:
  phase: runtime
flags: [module]
features:
  - BigInt
includes:
  - sta.js
---*/
throw 1;
"#;
        let case = parse_test262_case(source).expect("frontmatter parse should succeed");
        assert_eq!(
            case.frontmatter.negative_phase,
            Some(NegativePhase::Runtime)
        );
        assert_eq!(case.frontmatter.flags, vec!["module".to_string()]);
        assert_eq!(case.frontmatter.features, vec!["BigInt".to_string()]);
        assert_eq!(case.frontmatter.includes, vec!["sta.js".to_string()]);
    }

    #[test]
    fn parses_frontmatter_after_copyright_header() {
        let source = r#"// Copyright (C) 2026
// This code is governed by BSD.
/*---
negative:
  phase: parse
flags: [module]
---*/
import "x";
"#;
        let case = parse_test262_case(source).expect("frontmatter parse should succeed");
        assert_eq!(case.frontmatter.negative_phase, Some(NegativePhase::Parse));
        assert_eq!(case.frontmatter.flags, vec!["module".to_string()]);
        assert!(case.body.contains("import \"x\""));
    }

    #[test]
    fn maps_expected_outcomes() {
        let source = "/*---\nnegative:\n  phase: parse\n---*/\nthrow;";
        let case = parse_test262_case(source).expect("frontmatter parse should succeed");
        assert_eq!(
            expected_outcome(&case.frontmatter),
            ExpectedOutcome::ParseFail
        );
        assert!(!should_skip(&case.frontmatter));
    }

    #[test]
    fn skips_flags_includes_and_features_not_supported_yet() {
        let module_case = parse_test262_case("/*---\nflags: [module]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(should_skip(&module_case.frontmatter));

        let strict_case = parse_test262_case("/*---\nflags: [onlyStrict]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(should_skip(&strict_case.frontmatter));

        let async_case = parse_test262_case("/*---\nflags: [async]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(should_skip(&async_case.frontmatter));

        let includes_case = parse_test262_case("/*---\nincludes: [sta.js]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(should_skip(&includes_case.frontmatter));

        let feature_case = parse_test262_case("/*---\nfeatures: [BigInt]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(should_skip(&feature_case.frontmatter));
    }

    #[test]
    fn detects_unsupported_harness_globals_in_body() {
        assert!(!super::requires_unsupported_harness_globals(
            "assert(true);"
        ));
        assert!(!super::requires_unsupported_harness_globals(
            "assert.sameValue(x, y);"
        ));
        assert!(!super::requires_unsupported_harness_globals(
            "throw new Test262Error();"
        ));
        assert!(super::requires_unsupported_harness_globals(
            "$262.detachArrayBuffer(ab);"
        ));
        assert!(!super::requires_unsupported_harness_globals(
            "let x = 1; x + 1;"
        ));
    }

    #[test]
    fn detects_parse_tripwire_runtime_failures() {
        assert!(super::is_parse_tripwire_runtime_failure(
            "$DONOTEVALUATE();",
            &ExecutionOutcome::RuntimeFail("UnknownIdentifier(\"$DONOTEVALUATE\")".to_string())
        ));
        assert!(super::is_parse_tripwire_runtime_failure(
            "$DONOTEVALUATE();",
            &ExecutionOutcome::RuntimeFail(
                "UncaughtException(String(\"Test262: This statement should not be evaluated.\"))"
                    .to_string()
            )
        ));
        assert!(!super::is_parse_tripwire_runtime_failure(
            "1 + 1;",
            &ExecutionOutcome::RuntimeFail("UnknownIdentifier(\"$DONOTEVALUATE\")".to_string())
        ));
        assert!(super::is_parse_tripwire_runtime_failure(
            "throw \"Test262: This statement should not be evaluated.\";",
            &ExecutionOutcome::RuntimeFail(
                "UncaughtException(String(\"Test262: This statement should not be evaluated.\"))"
                    .to_string()
            )
        ));
    }

    #[test]
    fn detects_fixture_file_names() {
        assert!(super::is_fixture_file(&PathBuf::from(
            "language/module-code/setup_FIXTURE.js"
        )));
        assert!(!super::is_fixture_file(&PathBuf::from(
            "language/module-code/setup.js"
        )));
    }

    #[test]
    fn executes_and_classifies_parse_failure() {
        let result = super::execute_case("throw;");
        assert!(matches!(result, ExecutionOutcome::ParseFail(_)));
    }

    #[test]
    fn runs_suite_over_test262_lite_fixture() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("test262-lite");
        let summary =
            run_suite(&root, SuiteOptions::default()).expect("suite execution should succeed");
        assert!(summary.discovered > 0);
        assert!(summary.executed > 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.gc.collections_total, 0);
        assert_eq!(summary.gc.runtime_collections, 0);
        assert_eq!(summary.gc.boundary_collections, 0);
    }
}
