#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use test_harness::test262::{SuiteOptions, SuiteSummary, run_suite};

#[derive(Debug, Clone, Copy, Default)]
struct GcExpectations {
    collections_total_min: Option<usize>,
    runtime_collections_min: Option<usize>,
    runtime_ratio_min: Option<f64>,
    reclaimed_objects_min: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunProfile {
    Baseline,
    Stress,
}

impl RunProfile {
    fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Stress => "stress",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GcDriftStatus {
    Ok,
    Warning,
    Blocking,
}

impl GcDriftStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Blocking => "blocking",
        }
    }

    fn is_anomaly(self) -> bool {
        !matches!(self, Self::Ok)
    }
}

#[derive(Debug, Clone)]
struct GcDriftReport {
    profile: RunProfile,
    status: GcDriftStatus,
    warning_failures: Vec<String>,
    blocking_failures: Vec<String>,
    anomaly_streak: usize,
    investigation_required: bool,
}

fn merge_gc_expectations(base: GcExpectations, overrides: GcExpectations) -> GcExpectations {
    GcExpectations {
        collections_total_min: overrides
            .collections_total_min
            .or(base.collections_total_min),
        runtime_collections_min: overrides
            .runtime_collections_min
            .or(base.runtime_collections_min),
        runtime_ratio_min: overrides.runtime_ratio_min.or(base.runtime_ratio_min),
        reclaimed_objects_min: overrides
            .reclaimed_objects_min
            .or(base.reclaimed_objects_min),
    }
}

fn parse_runtime_ratio(value: &str, name: &str) -> Result<f64, String> {
    let ratio = value
        .parse::<f64>()
        .map_err(|_| format!("invalid {name} value: {value}"))?;
    if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
        return Err(format!(
            "{name} must be between 0.0 and 1.0 inclusive: {value}"
        ));
    }
    Ok(ratio)
}

fn parse_gc_expectations_str(raw: &str) -> Result<GcExpectations, String> {
    let mut expectations = GcExpectations::default();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (key, value) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("line {}: expected key=value", index + 1))?;
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            return Err(format!("line {}: missing value for key '{key}'", index + 1));
        }

        match key {
            "collections_total_min" => {
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| format!("line {}: invalid usize for '{key}'", index + 1))?;
                if expectations.collections_total_min.replace(parsed).is_some() {
                    return Err(format!("line {}: duplicate key '{key}'", index + 1));
                }
            }
            "runtime_collections_min" => {
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| format!("line {}: invalid usize for '{key}'", index + 1))?;
                if expectations
                    .runtime_collections_min
                    .replace(parsed)
                    .is_some()
                {
                    return Err(format!("line {}: duplicate key '{key}'", index + 1));
                }
            }
            "runtime_ratio_min" => {
                let parsed = parse_runtime_ratio(value, &format!("line {} '{key}'", index + 1))?;
                if expectations.runtime_ratio_min.replace(parsed).is_some() {
                    return Err(format!("line {}: duplicate key '{key}'", index + 1));
                }
            }
            "reclaimed_objects_min" => {
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| format!("line {}: invalid usize for '{key}'", index + 1))?;
                if expectations.reclaimed_objects_min.replace(parsed).is_some() {
                    return Err(format!("line {}: duplicate key '{key}'", index + 1));
                }
            }
            _ => {
                return Err(format!("line {}: unknown key '{key}'", index + 1));
            }
        }
    }
    Ok(expectations)
}

fn load_gc_expectations(path: &Path) -> Result<GcExpectations, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read gc baseline {}: {err}", path.display()))?;
    parse_gc_expectations_str(&raw)
        .map_err(|err| format!("failed to parse gc baseline {}: {err}", path.display()))
}

fn runtime_collection_ratio(runtime_collections: usize, collections_total: usize) -> f64 {
    if collections_total == 0 {
        0.0
    } else {
        runtime_collections as f64 / collections_total as f64
    }
}

fn check_gc_expectations(summary: &SuiteSummary, expectations: &GcExpectations) -> Vec<String> {
    let mut failures = Vec::new();
    let gc = &summary.gc;
    if gc.collections_total != gc.runtime_collections + gc.boundary_collections {
        failures.push(format!(
            "expected gc.collections_total == gc.runtime_collections + gc.boundary_collections, got {} != {} + {}",
            gc.collections_total, gc.runtime_collections, gc.boundary_collections
        ));
    }

    if let Some(min) = expectations.collections_total_min {
        if gc.collections_total < min {
            failures.push(format!(
                "expected gc.collections_total >= {min}, got {}",
                gc.collections_total
            ));
        }
    }

    if let Some(min) = expectations.runtime_collections_min {
        if gc.runtime_collections < min {
            failures.push(format!(
                "expected gc.runtime_collections >= {min}, got {}",
                gc.runtime_collections
            ));
        }
    }

    if let Some(min) = expectations.runtime_ratio_min {
        let ratio = runtime_collection_ratio(gc.runtime_collections, gc.collections_total);
        if ratio < min {
            if gc.collections_total == 0 {
                failures.push(format!(
                    "expected gc.runtime_ratio >= {min:.4}, got {ratio:.4} (runtime_collections={}, collections_total=0)",
                    gc.runtime_collections
                ));
            } else {
                failures.push(format!(
                    "expected gc.runtime_ratio >= {min:.4}, got {ratio:.4} (runtime_collections={}, collections_total={})",
                    gc.runtime_collections, gc.collections_total
                ));
            }
        }
    }

    if let Some(min) = expectations.reclaimed_objects_min {
        if gc.reclaimed_objects < min {
            failures.push(format!(
                "expected gc.reclaimed_objects >= {min}, got {}",
                gc.reclaimed_objects
            ));
        }
    }

    failures
}

fn parse_profile(raw: &str) -> Result<RunProfile, String> {
    match raw {
        "baseline" => Ok(RunProfile::Baseline),
        "stress" => Ok(RunProfile::Stress),
        other => Err(format!(
            "invalid --profile value: {other} (expected baseline|stress)"
        )),
    }
}

fn derive_warning_expectations(blocking: &GcExpectations) -> GcExpectations {
    let relaxed_ratio = blocking
        .runtime_ratio_min
        .map(|value| (value - 0.05).max(0.0));
    GcExpectations {
        collections_total_min: blocking
            .collections_total_min
            .map(|value| value.saturating_sub(value / 10)),
        runtime_collections_min: blocking
            .runtime_collections_min
            .map(|value| value.saturating_sub(value / 10)),
        runtime_ratio_min: relaxed_ratio,
        reclaimed_objects_min: blocking
            .reclaimed_objects_min
            .map(|value| value.saturating_sub(value / 10)),
    }
}

fn parse_previous_gc_drift(path: &Path) -> Option<(GcDriftStatus, usize)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let gc_drift_start = raw.find("\"gc_drift\"")?;
    let gc_section = &raw[gc_drift_start..];
    let status_key = "\"status\": \"";
    let status_index = gc_section.find(status_key)?;
    let status_start = status_index + status_key.len();
    let status_end = gc_section[status_start..].find('"')?;
    let status_raw = &gc_section[status_start..status_start + status_end];
    let status = match status_raw {
        "ok" => GcDriftStatus::Ok,
        "warning" => GcDriftStatus::Warning,
        "blocking" => GcDriftStatus::Blocking,
        _ => return None,
    };

    let streak_key = "\"anomaly_streak\":";
    let streak_index = gc_section.find(streak_key)?;
    let streak_start = streak_index + streak_key.len();
    let streak_slice = gc_section[streak_start..].trim_start();
    let streak_text = streak_slice
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    let anomaly_streak = streak_text.parse::<usize>().ok()?;
    Some((status, anomaly_streak))
}

fn classify_gc_drift(
    summary: &SuiteSummary,
    profile: RunProfile,
    blocking_expectations: &GcExpectations,
    previous: Option<(GcDriftStatus, usize)>,
) -> GcDriftReport {
    let mut warning_failures = Vec::new();
    let mut blocking_failures = Vec::new();

    match profile {
        RunProfile::Baseline => {
            if summary.gc.collections_total
                != summary.gc.runtime_collections + summary.gc.boundary_collections
            {
                blocking_failures.push(format!(
                    "expected gc.collections_total == gc.runtime_collections + gc.boundary_collections, got {} != {} + {}",
                    summary.gc.collections_total,
                    summary.gc.runtime_collections,
                    summary.gc.boundary_collections
                ));
            }
            if summary.gc.collections_total > 0 {
                warning_failures.push(format!(
                    "baseline profile expected gc.collections_total == 0, got {}",
                    summary.gc.collections_total
                ));
            }
            if summary.gc.runtime_collections > 0 {
                blocking_failures.push(format!(
                    "baseline profile expected gc.runtime_collections == 0, got {}",
                    summary.gc.runtime_collections
                ));
            }
        }
        RunProfile::Stress => {
            blocking_failures = check_gc_expectations(summary, blocking_expectations);
            let warning_expectations = derive_warning_expectations(blocking_expectations);
            warning_failures = check_gc_expectations(summary, &warning_expectations);
        }
    }

    let status = if !blocking_failures.is_empty() {
        GcDriftStatus::Blocking
    } else if !warning_failures.is_empty() {
        GcDriftStatus::Warning
    } else {
        GcDriftStatus::Ok
    };

    let (previous_status, previous_streak) = previous.unwrap_or((GcDriftStatus::Ok, 0));
    let anomaly_streak = if status.is_anomaly() {
        if previous_status.is_anomaly() {
            previous_streak + 1
        } else {
            1
        }
    } else {
        0
    };
    let investigation_required = anomaly_streak >= 2;

    GcDriftReport {
        profile,
        status,
        warning_failures,
        blocking_failures,
        anomaly_streak,
        investigation_required,
    }
}

fn escape_json_string(input: &str) -> String {
    let mut escaped = String::new();
    for ch in input.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn format_string_array_json(items: &[String], indent: &str) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let mut output = String::new();
    output.push_str("[\n");
    for (index, item) in items.iter().enumerate() {
        output.push_str(indent);
        output.push('"');
        output.push_str(&escape_json_string(item));
        output.push('"');
        if index + 1 != items.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str(&indent[..indent.len().saturating_sub(2)]);
    output.push(']');
    output
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut root: Option<PathBuf> = None;
    let mut profile: Option<RunProfile> = None;
    let mut previous_summary_path: Option<PathBuf> = None;
    let mut max_cases: Option<usize> = None;
    let mut fail_fast = false;
    let mut allow_failures = false;
    let mut json: Option<PathBuf> = None;
    let mut markdown: Option<PathBuf> = None;
    let mut show_failures = 0usize;
    let mut auto_gc = false;
    let mut auto_gc_threshold: Option<usize> = None;
    let mut runtime_gc = false;
    let mut runtime_gc_check_interval: Option<usize> = None;
    let mut show_gc = false;
    let mut gc_baseline_path: Option<PathBuf> = None;
    let mut gc_expectation_overrides = GcExpectations::default();

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--root requires a path argument");
                });
                root = Some(PathBuf::from(value));
            }
            "--profile" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--profile requires a value (baseline|stress)");
                });
                profile = Some(parse_profile(value).unwrap_or_else(|err| panic!("{err}")));
            }
            "--previous-summary" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--previous-summary requires a path argument");
                });
                previous_summary_path = Some(PathBuf::from(value));
            }
            "--max-cases" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--max-cases requires an integer argument");
                });
                max_cases = Some(
                    value
                        .parse::<usize>()
                        .unwrap_or_else(|_| panic!("invalid --max-cases value: {value}")),
                );
            }
            "--fail-fast" => {
                fail_fast = true;
            }
            "--allow-failures" => {
                allow_failures = true;
            }
            "--json" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--json requires a path argument");
                });
                json = Some(PathBuf::from(value));
            }
            "--markdown" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--markdown requires a path argument");
                });
                markdown = Some(PathBuf::from(value));
            }
            "--show-failures" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--show-failures requires an integer argument");
                });
                show_failures = value
                    .parse::<usize>()
                    .unwrap_or_else(|_| panic!("invalid --show-failures value: {value}"));
            }
            "--auto-gc" => {
                auto_gc = true;
            }
            "--auto-gc-threshold" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--auto-gc-threshold requires an integer argument");
                });
                auto_gc_threshold = Some(
                    value
                        .parse::<usize>()
                        .unwrap_or_else(|_| panic!("invalid --auto-gc-threshold value: {value}")),
                );
            }
            "--runtime-gc" => {
                runtime_gc = true;
            }
            "--runtime-gc-interval" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--runtime-gc-interval requires an integer argument");
                });
                runtime_gc_check_interval =
                    Some(value.parse::<usize>().unwrap_or_else(|_| {
                        panic!("invalid --runtime-gc-interval value: {value}")
                    }));
            }
            "--show-gc" => {
                show_gc = true;
            }
            "--expect-collections-total-min" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-collections-total-min requires an integer argument");
                });
                gc_expectation_overrides.collections_total_min =
                    Some(value.parse::<usize>().unwrap_or_else(|_| {
                        panic!("invalid --expect-collections-total-min value: {value}")
                    }));
            }
            "--expect-runtime-collections-min" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-runtime-collections-min requires an integer argument");
                });
                gc_expectation_overrides.runtime_collections_min =
                    Some(value.parse::<usize>().unwrap_or_else(|_| {
                        panic!("invalid --expect-runtime-collections-min value: {value}")
                    }));
            }
            "--expect-runtime-ratio-min" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-runtime-ratio-min requires a float argument");
                });
                gc_expectation_overrides.runtime_ratio_min = Some(
                    parse_runtime_ratio(value, "--expect-runtime-ratio-min")
                        .unwrap_or_else(|err| panic!("{err}")),
                );
            }
            "--expect-reclaimed-objects-min" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-reclaimed-objects-min requires an integer argument");
                });
                gc_expectation_overrides.reclaimed_objects_min =
                    Some(value.parse::<usize>().unwrap_or_else(|_| {
                        panic!("invalid --expect-reclaimed-objects-min value: {value}")
                    }));
            }
            "--expect-gc-baseline" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-gc-baseline requires a path argument");
                });
                gc_baseline_path = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                panic!("unknown argument: {other}");
            }
        }
        i += 1;
    }

    let root = root.unwrap_or_else(|| {
        panic!("missing required --root <path>");
    });
    let baseline_expectations = gc_baseline_path
        .as_deref()
        .map(|path| load_gc_expectations(path).unwrap_or_else(|err| panic!("{err}")))
        .unwrap_or_default();
    let gc_expectations = merge_gc_expectations(baseline_expectations, gc_expectation_overrides);
    let profile = profile.unwrap_or_else(|| {
        if runtime_gc || auto_gc {
            RunProfile::Stress
        } else {
            RunProfile::Baseline
        }
    });
    let previous_gc_drift = previous_summary_path
        .as_deref()
        .and_then(parse_previous_gc_drift);

    let options = SuiteOptions {
        max_cases,
        fail_fast,
        failure_details_limit: show_failures,
        auto_gc,
        auto_gc_threshold,
        runtime_gc,
        runtime_gc_check_interval,
    };

    let summary = run_suite(&root, options).unwrap_or_else(|err| {
        panic!("suite run failed: {err}");
    });
    let gc_drift = classify_gc_drift(&summary, profile, &gc_expectations, previous_gc_drift);

    print_summary(&summary, &gc_drift, show_gc);
    if !gc_drift.warning_failures.is_empty() {
        println!("gc warning drift signals:");
        for failure in &gc_drift.warning_failures {
            println!("  - {failure}");
        }
    }
    if !gc_drift.blocking_failures.is_empty() {
        println!("gc blocking drift signals:");
        for failure in &gc_drift.blocking_failures {
            println!("  - {failure}");
        }
    }

    if let Some(path) = json {
        write_summary_json(&path, &summary, &gc_drift).unwrap_or_else(|err| {
            panic!("failed to write json summary to {}: {err}", path.display());
        });
    }
    if let Some(path) = markdown {
        write_summary_markdown(&path, &summary, &gc_drift).unwrap_or_else(|err| {
            panic!(
                "failed to write markdown summary to {}: {err}",
                path.display()
            );
        });
    }

    if !summary.failures.is_empty() {
        println!("sample failures:");
        for detail in &summary.failures {
            println!("  - {}", detail.path);
            println!("    expected: {:?}", detail.expected);
            println!("    actual:   {:?}", detail.actual);
        }
    }

    if (summary.failed > 0 && !allow_failures)
        || gc_drift.status == GcDriftStatus::Blocking
        || gc_drift.investigation_required
    {
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "Usage: cargo run -p test-harness --bin test262-run -- --root <path> [--profile baseline|stress] [--previous-summary <path>] [--max-cases N] [--fail-fast] [--allow-failures] [--json <path>] [--markdown <path>] [--show-failures N] [--auto-gc] [--auto-gc-threshold N] [--runtime-gc] [--runtime-gc-interval N] [--show-gc] [--expect-gc-baseline <path>] [--expect-collections-total-min N] [--expect-runtime-collections-min N] [--expect-runtime-ratio-min R] [--expect-reclaimed-objects-min N]"
    );
}

fn print_summary(summary: &SuiteSummary, gc_drift: &GcDriftReport, show_gc: bool) {
    println!("test262 summary:");
    println!("  profile:    {}", gc_drift.profile.as_str());
    println!("  discovered: {}", summary.discovered);
    println!("  executed:   {}", summary.executed);
    println!("  skipped:    {}", summary.skipped);
    println!("  passed:     {}", summary.passed);
    println!("  failed:     {}", summary.failed);
    println!("  gc_drift_status: {}", gc_drift.status.as_str());
    println!("  anomaly_streak: {}", gc_drift.anomaly_streak);
    println!(
        "  investigation_required: {}",
        gc_drift.investigation_required
    );
    println!("  skipped categories:");
    for (name, value) in skip_category_rows(summary) {
        println!("    {name}: {value}");
    }
    if show_gc {
        println!("gc summary:");
        println!("  collections_total: {}", summary.gc.collections_total);
        println!(
            "  boundary_collections: {}",
            summary.gc.boundary_collections
        );
        println!("  runtime_collections: {}", summary.gc.runtime_collections);
        println!("  reclaimed_objects: {}", summary.gc.reclaimed_objects);
        println!("  mark_duration_ns: {}", summary.gc.mark_duration_ns);
        println!("  sweep_duration_ns: {}", summary.gc.sweep_duration_ns);
    }
}

fn skip_category_rows(summary: &SuiteSummary) -> [(&'static str, usize); 7] {
    [
        ("fixture_file", summary.skipped_categories.fixture_file),
        ("flag_module", summary.skipped_categories.flag_module),
        (
            "flag_only_strict",
            summary.skipped_categories.flag_only_strict,
        ),
        ("flag_async", summary.skipped_categories.flag_async),
        (
            "requires_includes",
            summary.skipped_categories.requires_includes,
        ),
        (
            "requires_feature",
            summary.skipped_categories.requires_feature,
        ),
        (
            "requires_harness_global_262",
            summary.skipped_categories.requires_harness_global_262,
        ),
    ]
}

fn format_summary_json(summary: &SuiteSummary, gc_drift: &GcDriftReport) -> String {
    let warning_failures = format_string_array_json(&gc_drift.warning_failures, "      ");
    let blocking_failures = format_string_array_json(&gc_drift.blocking_failures, "      ");
    format!(
        "{{\n  \"profile\": \"{}\",\n  \"discovered\": {},\n  \"executed\": {},\n  \"skipped\": {},\n  \"passed\": {},\n  \"failed\": {},\n  \"skipped_categories\": {{\n    \"fixture_file\": {},\n    \"flag_module\": {},\n    \"flag_only_strict\": {},\n    \"flag_async\": {},\n    \"requires_includes\": {},\n    \"requires_feature\": {},\n    \"requires_harness_global_262\": {}\n  }},\n  \"gc\": {{\n    \"collections_total\": {},\n    \"boundary_collections\": {},\n    \"runtime_collections\": {},\n    \"reclaimed_objects\": {},\n    \"mark_duration_ns\": {},\n    \"sweep_duration_ns\": {}\n  }},\n  \"gc_drift\": {{\n    \"status\": \"{}\",\n    \"anomaly_streak\": {},\n    \"investigation_required\": {},\n    \"warning_failures\": {},\n    \"blocking_failures\": {}\n  }}\n}}\n",
        gc_drift.profile.as_str(),
        summary.discovered,
        summary.executed,
        summary.skipped,
        summary.passed,
        summary.failed,
        summary.skipped_categories.fixture_file,
        summary.skipped_categories.flag_module,
        summary.skipped_categories.flag_only_strict,
        summary.skipped_categories.flag_async,
        summary.skipped_categories.requires_includes,
        summary.skipped_categories.requires_feature,
        summary.skipped_categories.requires_harness_global_262,
        summary.gc.collections_total,
        summary.gc.boundary_collections,
        summary.gc.runtime_collections,
        summary.gc.reclaimed_objects,
        summary.gc.mark_duration_ns,
        summary.gc.sweep_duration_ns,
        gc_drift.status.as_str(),
        gc_drift.anomaly_streak,
        gc_drift.investigation_required,
        warning_failures,
        blocking_failures
    )
}

fn format_summary_markdown(summary: &SuiteSummary, gc_drift: &GcDriftReport) -> String {
    let mut markdown = String::new();
    markdown.push_str("# Test262 Summary Report\n\n");
    markdown.push_str("## Profile\n\n");
    markdown.push_str("| Field | Value |\n");
    markdown.push_str("| --- | --- |\n");
    markdown.push_str(&format!("| profile | {} |\n", gc_drift.profile.as_str()));
    markdown.push_str(&format!(
        "| gc_drift_status | {} |\n",
        gc_drift.status.as_str()
    ));
    markdown.push_str(&format!(
        "| anomaly_streak | {} |\n",
        gc_drift.anomaly_streak
    ));
    markdown.push_str(&format!(
        "| investigation_required | {} |\n\n",
        gc_drift.investigation_required
    ));

    markdown.push_str("## Totals\n\n");
    markdown.push_str("| Metric | Value |\n");
    markdown.push_str("| --- | ---: |\n");
    markdown.push_str(&format!("| discovered | {} |\n", summary.discovered));
    markdown.push_str(&format!("| executed | {} |\n", summary.executed));
    markdown.push_str(&format!("| skipped | {} |\n", summary.skipped));
    markdown.push_str(&format!("| passed | {} |\n", summary.passed));
    markdown.push_str(&format!("| failed | {} |\n\n", summary.failed));

    markdown.push_str("## Skipped Categories\n\n");
    markdown.push_str("| Category | Count |\n");
    markdown.push_str("| --- | ---: |\n");
    for (name, value) in skip_category_rows(summary) {
        markdown.push_str(&format!("| {name} | {value} |\n"));
    }
    markdown.push_str(&format!("| total | {} |\n\n", summary.skipped));

    markdown.push_str("## GC Summary\n\n");
    markdown.push_str("| Metric | Value |\n");
    markdown.push_str("| --- | ---: |\n");
    markdown.push_str(&format!(
        "| collections_total | {} |\n",
        summary.gc.collections_total
    ));
    markdown.push_str(&format!(
        "| boundary_collections | {} |\n",
        summary.gc.boundary_collections
    ));
    markdown.push_str(&format!(
        "| runtime_collections | {} |\n",
        summary.gc.runtime_collections
    ));
    markdown.push_str(&format!(
        "| reclaimed_objects | {} |\n",
        summary.gc.reclaimed_objects
    ));
    markdown.push_str(&format!(
        "| mark_duration_ns | {} |\n",
        summary.gc.mark_duration_ns
    ));
    markdown.push_str(&format!(
        "| sweep_duration_ns | {} |\n",
        summary.gc.sweep_duration_ns
    ));
    markdown.push_str("\n## GC Drift Signals\n\n");
    markdown.push_str("### Warning\n\n");
    if gc_drift.warning_failures.is_empty() {
        markdown.push_str("- none\n");
    } else {
        for failure in &gc_drift.warning_failures {
            markdown.push_str(&format!("- {failure}\n"));
        }
    }
    markdown.push_str("\n### Blocking\n\n");
    if gc_drift.blocking_failures.is_empty() {
        markdown.push_str("- none\n");
    } else {
        for failure in &gc_drift.blocking_failures {
            markdown.push_str(&format!("- {failure}\n"));
        }
    }
    markdown
}

fn write_summary_json(
    path: &Path,
    summary: &SuiteSummary,
    gc_drift: &GcDriftReport,
) -> Result<(), String> {
    write_output(path, &format_summary_json(summary, gc_drift))
}

fn write_summary_markdown(
    path: &Path,
    summary: &SuiteSummary,
    gc_drift: &GcDriftReport,
) -> Result<(), String> {
    write_output(path, &format_summary_markdown(summary, gc_drift))
}

fn write_output(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    std::fs::write(path, content).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        GcDriftReport, GcDriftStatus, GcExpectations, RunProfile, check_gc_expectations,
        classify_gc_drift, format_summary_json, format_summary_markdown, merge_gc_expectations,
        parse_gc_expectations_str, skip_category_rows,
    };
    use test_harness::test262::{SuiteSkipCategories, SuiteSummary};

    fn summary_with_gc(
        collections_total: usize,
        runtime_collections: usize,
        reclaimed_objects: usize,
    ) -> SuiteSummary {
        let mut summary = SuiteSummary::default();
        summary.gc.collections_total = collections_total;
        summary.gc.runtime_collections = runtime_collections;
        summary.gc.boundary_collections = collections_total.saturating_sub(runtime_collections);
        summary.gc.reclaimed_objects = reclaimed_objects;
        summary
    }

    fn summary_with_phase7_counts() -> SuiteSummary {
        let mut summary = summary_with_gc(10, 7, 50);
        summary.discovered = 22;
        summary.executed = 14;
        summary.passed = 13;
        summary.failed = 1;
        summary.skipped = 8;
        summary.skipped_categories = SuiteSkipCategories {
            fixture_file: 1,
            flag_module: 1,
            flag_only_strict: 1,
            flag_async: 1,
            requires_includes: 1,
            requires_feature: 1,
            requires_harness_global_262: 2,
        };
        summary
    }

    fn ok_gc_drift(profile: RunProfile) -> GcDriftReport {
        GcDriftReport {
            profile,
            status: GcDriftStatus::Ok,
            warning_failures: Vec::new(),
            blocking_failures: Vec::new(),
            anomaly_streak: 0,
            investigation_required: false,
        }
    }

    #[test]
    fn gc_expectations_pass_when_thresholds_are_met() {
        let summary = summary_with_gc(10, 4, 50);
        let expectations = GcExpectations {
            collections_total_min: Some(8),
            runtime_collections_min: Some(3),
            runtime_ratio_min: Some(0.4),
            reclaimed_objects_min: Some(40),
        };

        let failures = check_gc_expectations(&summary, &expectations);
        assert!(failures.is_empty());
    }

    #[test]
    fn gc_runtime_ratio_handles_zero_total_collections() {
        let summary = summary_with_gc(0, 0, 0);
        let expectations = GcExpectations {
            collections_total_min: None,
            runtime_collections_min: None,
            runtime_ratio_min: Some(0.1),
            reclaimed_objects_min: None,
        };

        let failures = check_gc_expectations(&summary, &expectations);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("gc.runtime_ratio"));
        assert!(failures[0].contains("collections_total=0"));
    }

    #[test]
    fn gc_expectations_fail_when_reclaimed_objects_is_below_minimum() {
        let summary = summary_with_gc(8, 2, 9);
        let expectations = GcExpectations {
            collections_total_min: None,
            runtime_collections_min: None,
            runtime_ratio_min: None,
            reclaimed_objects_min: Some(10),
        };

        let failures = check_gc_expectations(&summary, &expectations);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("gc.reclaimed_objects"));
    }

    #[test]
    fn parses_gc_baseline_file_content() {
        let raw = r#"
# comment
collections_total_min=1000
runtime_collections_min=1000
runtime_ratio_min=0.90
reclaimed_objects_min=1
"#;
        let expectations = parse_gc_expectations_str(raw).expect("baseline should parse");
        assert_eq!(expectations.collections_total_min, Some(1000));
        assert_eq!(expectations.runtime_collections_min, Some(1000));
        assert_eq!(expectations.runtime_ratio_min, Some(0.90));
        assert_eq!(expectations.reclaimed_objects_min, Some(1));
    }

    #[test]
    fn rejects_unknown_gc_baseline_key() {
        let raw = "unknown_key=1";
        let err = parse_gc_expectations_str(raw).expect_err("baseline parse should fail");
        assert!(err.contains("unknown key"));
    }

    #[test]
    fn rejects_duplicate_gc_baseline_key() {
        let raw = "collections_total_min=100\ncollections_total_min=200\n";
        let err = parse_gc_expectations_str(raw).expect_err("baseline parse should fail");
        assert_eq!(err, "line 2: duplicate key 'collections_total_min'");
    }

    #[test]
    fn rejects_out_of_range_runtime_ratio_in_baseline() {
        let raw = "runtime_ratio_min=1.5";
        let err = parse_gc_expectations_str(raw).expect_err("baseline parse should fail");
        assert_eq!(
            err,
            "line 1 'runtime_ratio_min' must be between 0.0 and 1.0 inclusive: 1.5"
        );
    }

    #[test]
    fn explicit_expectation_overrides_take_precedence_over_baseline() {
        let baseline = GcExpectations {
            collections_total_min: Some(1000),
            runtime_collections_min: Some(1000),
            runtime_ratio_min: Some(0.9),
            reclaimed_objects_min: Some(1),
        };
        let overrides = GcExpectations {
            collections_total_min: Some(2000),
            runtime_collections_min: None,
            runtime_ratio_min: Some(0.95),
            reclaimed_objects_min: Some(5),
        };
        let merged = merge_gc_expectations(baseline, overrides);
        assert_eq!(merged.collections_total_min, Some(2000));
        assert_eq!(merged.runtime_collections_min, Some(1000));
        assert_eq!(merged.runtime_ratio_min, Some(0.95));
        assert_eq!(merged.reclaimed_objects_min, Some(5));
    }

    #[test]
    fn gc_expectations_fail_when_collection_accounting_is_imbalanced() {
        let mut summary = summary_with_gc(10, 4, 1);
        summary.gc.boundary_collections = 3;
        let expectations = GcExpectations::default();
        let failures = check_gc_expectations(&summary, &expectations);
        assert_eq!(
            failures,
            vec![
                "expected gc.collections_total == gc.runtime_collections + gc.boundary_collections, got 10 != 4 + 3",
            ]
        );
    }

    #[test]
    fn gc_expectations_emit_deterministic_failure_messages_for_all_threshold_misses() {
        let mut summary = summary_with_gc(5, 2, 0);
        summary.gc.boundary_collections = 1;
        let expectations = GcExpectations {
            collections_total_min: Some(10),
            runtime_collections_min: Some(4),
            runtime_ratio_min: Some(0.8),
            reclaimed_objects_min: Some(1),
        };

        let failures = check_gc_expectations(&summary, &expectations);
        assert_eq!(
            failures,
            vec![
                "expected gc.collections_total == gc.runtime_collections + gc.boundary_collections, got 5 != 2 + 1",
                "expected gc.collections_total >= 10, got 5",
                "expected gc.runtime_collections >= 4, got 2",
                "expected gc.runtime_ratio >= 0.8000, got 0.4000 (runtime_collections=2, collections_total=5)",
                "expected gc.reclaimed_objects >= 1, got 0",
            ]
        );
    }

    #[test]
    fn json_report_schema_includes_phase7_required_fields() {
        let summary = summary_with_phase7_counts();
        let json = format_summary_json(&summary, &ok_gc_drift(RunProfile::Stress));

        assert!(json.contains("\"profile\": \"stress\""));
        assert!(json.contains("\"discovered\": 22"));
        assert!(json.contains("\"executed\": 14"));
        assert!(json.contains("\"failed\": 1"));
        assert!(json.contains("\"skipped_categories\": {"));
        assert!(json.contains("\"fixture_file\": 1"));
        assert!(json.contains("\"flag_module\": 1"));
        assert!(json.contains("\"flag_only_strict\": 1"));
        assert!(json.contains("\"flag_async\": 1"));
        assert!(json.contains("\"requires_includes\": 1"));
        assert!(json.contains("\"requires_feature\": 1"));
        assert!(json.contains("\"requires_harness_global_262\": 2"));
        assert!(json.contains("\"gc_drift\": {"));
        assert!(json.contains("\"status\": \"ok\""));
    }

    #[test]
    fn markdown_report_contains_deterministic_phase7_sections() {
        let summary = summary_with_phase7_counts();
        let markdown = format_summary_markdown(&summary, &ok_gc_drift(RunProfile::Baseline));

        let profile_index = markdown
            .find("## Profile")
            .expect("markdown should include Profile heading");
        let totals_index = markdown
            .find("## Totals")
            .expect("markdown should include Totals heading");
        let skipped_index = markdown
            .find("## Skipped Categories")
            .expect("markdown should include Skipped Categories heading");
        let gc_index = markdown
            .find("## GC Summary")
            .expect("markdown should include GC Summary heading");
        assert!(profile_index < totals_index);
        assert!(totals_index < skipped_index);
        assert!(skipped_index < gc_index);

        assert!(markdown.contains("| profile | baseline |"));
        assert!(markdown.contains("| discovered | 22 |"));
        assert!(markdown.contains("| executed | 14 |"));
        assert!(markdown.contains("| failed | 1 |"));
        assert!(markdown.contains("| fixture_file | 1 |"));
        assert!(markdown.contains("| requires_harness_global_262 | 2 |"));
        assert!(markdown.contains("| total | 8 |"));
    }

    #[test]
    fn skipped_category_rows_sum_to_skipped_total() {
        let summary = summary_with_phase7_counts();
        let categorized_total: usize = skip_category_rows(&summary)
            .into_iter()
            .map(|(_, count)| count)
            .sum();
        assert_eq!(summary.skipped, categorized_total);
    }

    #[test]
    fn classify_gc_drift_marks_baseline_runtime_gc_as_blocking() {
        let mut summary = summary_with_gc(1, 1, 0);
        summary.gc.boundary_collections = 0;
        let drift = classify_gc_drift(
            &summary,
            RunProfile::Baseline,
            &GcExpectations::default(),
            None,
        );
        assert_eq!(drift.status, GcDriftStatus::Blocking);
        assert_eq!(drift.anomaly_streak, 1);
        assert!(!drift.blocking_failures.is_empty());
    }

    #[test]
    fn classify_gc_drift_requires_investigation_on_consecutive_anomalies() {
        let summary = summary_with_gc(1, 0, 0);
        let drift = classify_gc_drift(
            &summary,
            RunProfile::Baseline,
            &GcExpectations::default(),
            Some((GcDriftStatus::Warning, 1)),
        );
        assert_eq!(drift.status, GcDriftStatus::Warning);
        assert_eq!(drift.anomaly_streak, 2);
        assert!(drift.investigation_required);
    }
}
