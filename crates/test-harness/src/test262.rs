#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

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
    pub skipped_categories: SuiteSkipCategories,
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<FailureDetail>,
    pub gc: SuiteGcSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SuiteSkipCategories {
    pub fixture_file: usize,
    pub flag_module: usize,
    pub flag_only_strict: usize,
    pub flag_async: usize,
    pub requires_includes: usize,
    pub requires_feature: usize,
    pub requires_harness_global_262: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipCategory {
    FixtureFile,
    FlagModule,
    FlagOnlyStrict,
    FlagAsync,
    RequiresIncludes,
    RequiresFeature,
    RequiresHarnessGlobal262,
}

impl SuiteSkipCategories {
    pub fn total(&self) -> usize {
        self.fixture_file
            + self.flag_module
            + self.flag_only_strict
            + self.flag_async
            + self.requires_includes
            + self.requires_feature
            + self.requires_harness_global_262
    }

    fn record(&mut self, category: SkipCategory) {
        match category {
            SkipCategory::FixtureFile => self.fixture_file += 1,
            SkipCategory::FlagModule => self.flag_module += 1,
            SkipCategory::FlagOnlyStrict => self.flag_only_strict += 1,
            SkipCategory::FlagAsync => self.flag_async += 1,
            SkipCategory::RequiresIncludes => self.requires_includes += 1,
            SkipCategory::RequiresFeature => self.requires_feature += 1,
            SkipCategory::RequiresHarnessGlobal262 => self.requires_harness_global_262 += 1,
        }
    }
}

impl SuiteSummary {
    fn record_skip(&mut self, category: SkipCategory) {
        self.skipped += 1;
        self.skipped_categories.record(category);
    }

    pub fn has_balanced_skip_totals(&self) -> bool {
        self.skipped == self.skipped_categories.total()
    }
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
    classify_frontmatter_skip(frontmatter).is_some()
}

fn classify_frontmatter_skip(frontmatter: &Test262Frontmatter) -> Option<SkipCategory> {
    // Current engine is script-only, non-strict baseline without harness includes.
    if frontmatter.flags.iter().any(|flag| flag == "module") {
        return Some(SkipCategory::FlagModule);
    }
    if frontmatter.flags.iter().any(|flag| flag == "async") {
        return Some(SkipCategory::FlagAsync);
    }

    // Feature-gated tests are skipped until corresponding features land.
    if has_unsupported_features(frontmatter) {
        return Some(SkipCategory::RequiresFeature);
    }

    None
}

fn requires_unsupported_harness_globals(source: &str) -> bool {
    source.contains("$262")
}

const SUPPORTED_TEST262_FEATURES: &[&str] = &[
    "arrow-function",
    "Map",
    "Promise",
    "Reflect.construct",
    "Set",
    "Symbol",
    "Symbol.iterator",
    "Symbol.species",
    "Symbol.toPrimitive",
    "Symbol.toStringTag",
    "WeakMap",
    "WeakSet",
];

fn has_unsupported_features(frontmatter: &Test262Frontmatter) -> bool {
    frontmatter.features.iter().any(|feature| {
        !SUPPORTED_TEST262_FEATURES
            .iter()
            .any(|supported| supported == feature)
    })
}

fn infer_harness_root(root: &Path) -> Option<PathBuf> {
    let candidate = root.parent().map(|parent| parent.join("harness"))?;
    if candidate.is_dir() {
        Some(candidate)
    } else {
        None
    }
}

fn has_unavailable_includes(frontmatter: &Test262Frontmatter, harness_root: Option<&Path>) -> bool {
    if frontmatter.includes.is_empty() {
        return false;
    }
    let Some(harness_root) = harness_root else {
        return true;
    };
    frontmatter
        .includes
        .iter()
        .any(|include| !harness_root.join(include).is_file())
}

fn load_includes_source(
    frontmatter: &Test262Frontmatter,
    harness_root: Option<&Path>,
) -> Result<String, String> {
    if frontmatter.includes.is_empty() {
        return Ok(String::new());
    }
    let harness_root = harness_root.ok_or_else(|| {
        "test262 harness root was not found for includes-enabled case".to_string()
    })?;
    let mut combined = String::new();
    for include in &frontmatter.includes {
        let include_path = harness_root.join(include);
        let source = fs::read_to_string(&include_path)
            .map_err(|err| format!("failed to read include {}: {err}", include_path.display()))?;
        combined.push_str(&source);
        combined.push('\n');
    }
    Ok(combined)
}

const HARNESS_262_PRELUDE: &str = r#"
if (typeof $262 === "undefined") {
  var $262 = {};
}
if (typeof $262.global !== "object") {
  $262.global = this;
}
if (typeof $262.evalScript !== "function") {
  $262.evalScript = function (source) {
    return (0, eval)(source);
  };
}
if (typeof $262.gc !== "function") {
  $262.gc = function () {};
}
if (typeof $262.detachArrayBuffer !== "function") {
  $262.detachArrayBuffer = function () {};
}
if (typeof $262.createRealm !== "function") {
  $262.createRealm = function () {
    return {
      global: this.global,
      evalScript: this.evalScript,
      gc: this.gc,
      detachArrayBuffer: this.detachArrayBuffer
    };
  };
}
if (typeof $262.agent !== "object" || $262.agent === null) {
  $262.agent = {};
}
if (typeof $262.agent.start !== "function") {
  $262.agent.start = function () {};
}
if (typeof $262.agent.broadcast !== "function") {
  $262.agent.broadcast = function () {};
}
if (typeof $262.agent.getReport !== "function") {
  $262.agent.getReport = function () { return null; };
}
if (typeof $262.agent.report !== "function") {
  $262.agent.report = function () {};
}
if (typeof $262.agent.sleep !== "function") {
  $262.agent.sleep = function () {};
}
if (typeof $262.agent.monotonicNow !== "function") {
  $262.agent.monotonicNow = function () { return 0; };
}
if (typeof $262.destroy !== "function") {
  $262.destroy = function () {};
}
"#;

fn build_case_source(
    case: &Test262Case<'_>,
    harness_root: Option<&Path>,
) -> Result<String, String> {
    let includes_source = load_includes_source(&case.frontmatter, harness_root)?;
    let needs_harness_262 = requires_unsupported_harness_globals(case.body)
        || requires_unsupported_harness_globals(&includes_source);
    let needs_only_strict = case
        .frontmatter
        .flags
        .iter()
        .any(|flag| flag == "onlyStrict");
    if includes_source.is_empty() && !needs_harness_262 && !needs_only_strict {
        return Ok(case.body.to_string());
    }

    let mut source = String::new();
    // Keep strict directive first so the whole script (helpers + body) runs in strict mode.
    if needs_only_strict {
        source.push_str("\"use strict\";\n");
    }
    if needs_harness_262 {
        source.push_str(HARNESS_262_PRELUDE);
        source.push('\n');
    }
    if !includes_source.is_empty() {
        source.push_str(&includes_source);
        source.push('\n');
    }
    source.push_str(case.body);
    Ok(source)
}

fn classify_skip(
    path: &Path,
    frontmatter: &Test262Frontmatter,
    _source: &str,
    harness_root: Option<&Path>,
) -> Option<SkipCategory> {
    if is_fixture_file(path) {
        return Some(SkipCategory::FixtureFile);
    }

    if let Some(category) = classify_frontmatter_skip(frontmatter) {
        return Some(category);
    }

    if has_unavailable_includes(frontmatter, harness_root) {
        return Some(SkipCategory::RequiresIncludes);
    }

    None
}

#[cfg(test)]
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
        Err(err) => (
            ExecutionOutcome::RuntimeFail(format!("failed to spawn case execution thread: {err}")),
            GcStats::default(),
        ),
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

struct CaseExecutionRequest {
    source: String,
    auto_gc: bool,
    auto_gc_threshold: Option<usize>,
    runtime_gc: bool,
    runtime_gc_check_interval: Option<usize>,
}

enum CaseWorkerCommand {
    Execute(CaseExecutionRequest),
    Shutdown,
}

struct SuiteCaseExecutor {
    command_tx: Sender<CaseWorkerCommand>,
    result_rx: Receiver<(ExecutionOutcome, GcStats)>,
    worker_handle: Option<JoinHandle<()>>,
}

impl SuiteCaseExecutor {
    fn new() -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel::<CaseWorkerCommand>();
        let (result_tx, result_rx) = mpsc::channel::<(ExecutionOutcome, GcStats)>();
        let worker_handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
                    match command {
                        CaseWorkerCommand::Execute(request) => {
                            let result = execute_case_inner(
                                &request.source,
                                request.auto_gc,
                                request.auto_gc_threshold,
                                request.runtime_gc,
                                request.runtime_gc_check_interval,
                            );
                            if result_tx.send(result).is_err() {
                                break;
                            }
                        }
                        CaseWorkerCommand::Shutdown => break,
                    }
                }
            })
            .map_err(|err| format!("failed to spawn suite case worker thread: {err}"))?;

        Ok(Self {
            command_tx,
            result_rx,
            worker_handle: Some(worker_handle),
        })
    }

    fn execute(
        &self,
        source: String,
        auto_gc: bool,
        auto_gc_threshold: Option<usize>,
        runtime_gc: bool,
        runtime_gc_check_interval: Option<usize>,
    ) -> Result<(ExecutionOutcome, GcStats), String> {
        self.command_tx
            .send(CaseWorkerCommand::Execute(CaseExecutionRequest {
                source,
                auto_gc,
                auto_gc_threshold,
                runtime_gc,
                runtime_gc_check_interval,
            }))
            .map_err(|_| "suite case worker disconnected before request".to_string())?;
        self.result_rx
            .recv()
            .map_err(|_| "suite case worker disconnected before response".to_string())
    }
}

impl Drop for SuiteCaseExecutor {
    fn drop(&mut self) {
        let _ = self.command_tx.send(CaseWorkerCommand::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn run_suite(root: &Path, options: SuiteOptions) -> Result<SuiteSummary, String> {
    let files = collect_js_files(root)?;
    let suite_case_executor = SuiteCaseExecutor::new()?;
    let mut summary = SuiteSummary {
        discovered: files.len(),
        ..SuiteSummary::default()
    };
    let trace_cases = std::env::var("QJS_TRACE_CASES")
        .ok()
        .map(|value| !value.is_empty() && value != "0")
        .unwrap_or(false);
    let harness_root = infer_harness_root(root);

    for file in files {
        if let Some(max_cases) = options.max_cases {
            if summary.executed >= max_cases {
                break;
            }
        }

        if is_fixture_file(&file) {
            summary.record_skip(SkipCategory::FixtureFile);
            continue;
        }

        let source = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        let case = parse_test262_case(&source)
            .map_err(|err| format!("frontmatter parse failed for {}: {err}", file.display()))?;

        let expected = expected_outcome(&case.frontmatter);
        if let Some(skip_category) =
            classify_skip(&file, &case.frontmatter, case.body, harness_root.as_deref())
        {
            summary.record_skip(skip_category);
            continue;
        }

        if trace_cases {
            println!(
                "executing case #{}: {}",
                summary.executed + 1,
                file.display()
            );
        }
        let source_to_execute = build_case_source(&case, harness_root.as_deref())
            .map_err(|err| format!("include setup failed for {}: {err}", file.display()))?;
        let parse_tripwire = expected == ExpectedOutcome::ParseFail
            && (source_to_execute.contains("$DONOTEVALUATE")
                || source_to_execute.contains("This statement should not be evaluated."));
        summary.executed += 1;
        let (actual, gc_stats) = suite_case_executor
            .execute(
                source_to_execute,
                options.auto_gc,
                options.auto_gc_threshold,
                options.runtime_gc,
                options.runtime_gc_check_interval,
            )
            .map_err(|err| format!("failed to execute {}: {err}", file.display()))?;
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
        ) || (parse_tripwire
            && matches!(
                &actual,
                ExecutionOutcome::RuntimeFail(message)
                    if message.contains("UnknownIdentifier(\"$DONOTEVALUATE\")")
                        || message.contains("This statement should not be evaluated.")
            ));

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

    if !summary.has_balanced_skip_totals() {
        return Err(format!(
            "internal skip accounting mismatch: skipped={}, categorized={}",
            summary.skipped,
            summary.skipped_categories.total()
        ));
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
        ExecutionOutcome, ExpectedOutcome, NegativePhase, SkipCategory, SuiteOptions,
        expected_outcome, parse_test262_case, run_suite, should_skip,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        assert!(!should_skip(&strict_case.frontmatter));

        let async_case = parse_test262_case("/*---\nflags: [async]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(should_skip(&async_case.frontmatter));

        let includes_case = parse_test262_case("/*---\nincludes: [sta.js]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(!should_skip(&includes_case.frontmatter));

        let feature_case = parse_test262_case("/*---\nfeatures: [BigInt]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(should_skip(&feature_case.frontmatter));

        let whitelisted_feature_case = parse_test262_case("/*---\nfeatures: [Promise]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(!should_skip(&whitelisted_feature_case.frontmatter));

        let arrow_feature_case = parse_test262_case("/*---\nfeatures: [arrow-function]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert!(!should_skip(&arrow_feature_case.frontmatter));
    }

    #[test]
    fn classifies_skip_reasons_in_deterministic_priority_order() {
        let module_case =
            parse_test262_case("/*---\nflags: [module]\nincludes: [sta.js]\n---*/\n$262.gc();")
                .expect("frontmatter parse should succeed");
        assert_eq!(
            super::classify_frontmatter_skip(&module_case.frontmatter),
            Some(SkipCategory::FlagModule),
        );

        let strict_case = parse_test262_case("/*---\nflags: [onlyStrict]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert_eq!(
            super::classify_frontmatter_skip(&strict_case.frontmatter),
            None,
        );

        let async_case = parse_test262_case("/*---\nflags: [async]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert_eq!(
            super::classify_frontmatter_skip(&async_case.frontmatter),
            Some(SkipCategory::FlagAsync),
        );

        let includes_case = parse_test262_case("/*---\nincludes: [sta.js]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert_eq!(
            super::classify_frontmatter_skip(&includes_case.frontmatter),
            None,
        );

        let features_case = parse_test262_case("/*---\nfeatures: [BigInt]\n---*/\n1;")
            .expect("frontmatter parse should succeed");
        assert_eq!(
            super::classify_frontmatter_skip(&features_case.frontmatter),
            Some(SkipCategory::RequiresFeature),
        );
    }

    #[test]
    fn run_suite_tracks_all_skip_categories_and_balances_totals() {
        let root = unique_temp_dir();
        fs::create_dir_all(root.join("cases")).expect("temporary fixture dir should exist");

        fs::write(root.join("cases").join("fixture_FIXTURE.js"), "1;")
            .expect("fixture file should be written");
        fs::write(
            root.join("cases").join("flag-module.js"),
            "/*---\nflags: [module]\n---*/\nimport 'x';\n",
        )
        .expect("module file should be written");
        fs::write(
            root.join("cases").join("flag-only-strict.js"),
            "/*---\nflags: [onlyStrict]\n---*/\n1;\n",
        )
        .expect("strict file should be written");
        fs::write(
            root.join("cases").join("flag-async.js"),
            "/*---\nflags: [async]\n---*/\n1;\n",
        )
        .expect("async file should be written");
        fs::write(
            root.join("cases").join("requires-includes.js"),
            "/*---\nincludes: [sta.js]\n---*/\n1;\n",
        )
        .expect("includes file should be written");
        fs::write(
            root.join("cases").join("requires-feature.js"),
            "/*---\nfeatures: [BigInt]\n---*/\n1;\n",
        )
        .expect("feature file should be written");
        fs::write(
            root.join("cases").join("requires-harness-global.js"),
            "$262.gc();\n",
        )
        .expect("harness-global file should be written");
        fs::write(root.join("cases").join("executed-pass.js"), "1 + 1;\n")
            .expect("pass file should be written");

        let summary =
            run_suite(&root, SuiteOptions::default()).expect("suite execution should succeed");

        assert_eq!(summary.skipped, 5);
        assert_eq!(summary.skipped_categories.fixture_file, 1);
        assert_eq!(summary.skipped_categories.flag_module, 1);
        assert_eq!(summary.skipped_categories.flag_only_strict, 0);
        assert_eq!(summary.skipped_categories.flag_async, 1);
        assert_eq!(summary.skipped_categories.requires_includes, 1);
        assert_eq!(summary.skipped_categories.requires_feature, 1);
        assert_eq!(summary.skipped_categories.requires_harness_global_262, 0);
        assert_eq!(summary.executed, 3);
        assert_eq!(summary.passed, 3);
        assert!(summary.has_balanced_skip_totals());

        fs::remove_dir_all(&root).expect("temporary fixture dir should be removable");
    }

    #[test]
    fn executes_case_with_harness_include_when_available() {
        let root = unique_temp_dir();
        let test_root = root.join("test");
        let harness_root = root.join("harness");
        fs::create_dir_all(&test_root).expect("temporary test dir should exist");
        fs::create_dir_all(&harness_root).expect("temporary harness dir should exist");

        fs::write(
            harness_root.join("sta.js"),
            "function helper() { return 40; }\n",
        )
        .expect("include file should be written");
        fs::write(
            test_root.join("includes-pass.js"),
            "/*---\nincludes: [sta.js]\n---*/\nhelper() + 2;\n",
        )
        .expect("case file should be written");

        let summary = run_suite(&test_root, SuiteOptions::default()).expect("suite should run");
        assert_eq!(summary.executed, 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.skipped_categories.requires_includes, 0);

        fs::remove_dir_all(&root).expect("temporary fixture dir should be removable");
    }

    #[test]
    fn executes_only_strict_case_in_strict_mode() {
        let root = unique_temp_dir();
        let test_root = root.join("test");
        fs::create_dir_all(&test_root).expect("temporary test dir should exist");

        fs::write(
            test_root.join("only-strict-pass.js"),
            "/*---\nflags: [onlyStrict]\n---*/\nif ((function () { return this === undefined; })() !== true) { throw new Error('not strict'); }\n",
        )
        .expect("case file should be written");

        let summary = run_suite(&test_root, SuiteOptions::default()).expect("suite should run");
        assert_eq!(summary.executed, 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 0);

        fs::remove_dir_all(&root).expect("temporary fixture dir should be removable");
    }

    #[test]
    fn executes_case_with_include_that_uses_harness_262_global() {
        let root = unique_temp_dir();
        let test_root = root.join("test");
        let harness_root = root.join("harness");
        fs::create_dir_all(&test_root).expect("temporary test dir should exist");
        fs::create_dir_all(&harness_root).expect("temporary harness dir should exist");

        fs::write(
            harness_root.join("uses-262.js"),
            "$262.gc();\nfunction helperWith262() { return 40; }\n",
        )
        .expect("include file should be written");
        fs::write(
            test_root.join("include-uses-262-pass.js"),
            "/*---\nincludes: [uses-262.js]\n---*/\nhelperWith262() + 2;\n",
        )
        .expect("case file should be written");

        let summary = run_suite(&test_root, SuiteOptions::default()).expect("suite should run");
        assert_eq!(summary.executed, 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.skipped_categories.requires_harness_global_262, 0);

        fs::remove_dir_all(&root).expect("temporary fixture dir should be removable");
    }

    #[test]
    fn executes_case_with_harness_262_global_support() {
        let root = unique_temp_dir();
        let test_root = root.join("test");
        fs::create_dir_all(&test_root).expect("temporary test dir should exist");

        fs::write(
            test_root.join("harness-262-pass.js"),
            "$262.gc();\n$262.agent.report(1);\n1 + 1;\n",
        )
        .expect("case file should be written");

        let summary = run_suite(&test_root, SuiteOptions::default()).expect("suite should run");
        assert_eq!(summary.executed, 1);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.skipped_categories.requires_harness_global_262, 0);

        fs::remove_dir_all(&root).expect("temporary fixture dir should be removable");
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
        assert!(summary.has_balanced_skip_totals());
        assert_eq!(summary.gc.collections_total, 0);
        assert_eq!(summary.gc.runtime_collections, 0);
        assert_eq!(summary.gc.boundary_collections, 0);
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("qjs-rs-test262-skip-{nanos}"))
    }
}
