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

fn merge_gc_expectations(base: GcExpectations, overrides: GcExpectations) -> GcExpectations {
    GcExpectations {
        collections_total_min: overrides.collections_total_min.or(base.collections_total_min),
        runtime_collections_min: overrides
            .runtime_collections_min
            .or(base.runtime_collections_min),
        runtime_ratio_min: overrides.runtime_ratio_min.or(base.runtime_ratio_min),
        reclaimed_objects_min: overrides.reclaimed_objects_min.or(base.reclaimed_objects_min),
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
                expectations.collections_total_min = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("line {}: invalid usize for '{key}'", index + 1))?,
                );
            }
            "runtime_collections_min" => {
                expectations.runtime_collections_min = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("line {}: invalid usize for '{key}'", index + 1))?,
                );
            }
            "runtime_ratio_min" => {
                expectations.runtime_ratio_min = Some(parse_runtime_ratio(
                    value,
                    &format!("line {} '{key}'", index + 1),
                )?);
            }
            "reclaimed_objects_min" => {
                expectations.reclaimed_objects_min = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("line {}: invalid usize for '{key}'", index + 1))?,
                );
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

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut root: Option<PathBuf> = None;
    let mut max_cases: Option<usize> = None;
    let mut fail_fast = false;
    let mut allow_failures = false;
    let mut json: Option<PathBuf> = None;
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
                gc_expectation_overrides.collections_total_min = Some(
                    value.parse::<usize>().unwrap_or_else(|_| {
                        panic!("invalid --expect-collections-total-min value: {value}")
                    }),
                );
            }
            "--expect-runtime-collections-min" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-runtime-collections-min requires an integer argument");
                });
                gc_expectation_overrides.runtime_collections_min = Some(
                    value.parse::<usize>().unwrap_or_else(|_| {
                        panic!("invalid --expect-runtime-collections-min value: {value}")
                    }),
                );
            }
            "--expect-runtime-ratio-min" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-runtime-ratio-min requires a float argument");
                });
                gc_expectation_overrides.runtime_ratio_min =
                    Some(parse_runtime_ratio(value, "--expect-runtime-ratio-min").unwrap_or_else(
                        |err| panic!("{err}"),
                    ));
            }
            "--expect-reclaimed-objects-min" => {
                i += 1;
                let value = args.get(i).unwrap_or_else(|| {
                    panic!("--expect-reclaimed-objects-min requires an integer argument");
                });
                gc_expectation_overrides.reclaimed_objects_min = Some(
                    value.parse::<usize>().unwrap_or_else(|_| {
                        panic!("invalid --expect-reclaimed-objects-min value: {value}")
                    }),
                );
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

    print_summary(&summary, show_gc);
    let gc_guard_failures = check_gc_expectations(&summary, &gc_expectations);
    if !gc_guard_failures.is_empty() {
        println!("gc guard failures:");
        for failure in &gc_guard_failures {
            println!("  - {failure}");
        }
    }

    if let Some(path) = json {
        write_summary_json(&path, &summary).unwrap_or_else(|err| {
            panic!("failed to write json summary to {}: {err}", path.display());
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

    if (summary.failed > 0 && !allow_failures) || !gc_guard_failures.is_empty() {
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "Usage: cargo run -p test-harness --bin test262-run -- --root <path> [--max-cases N] [--fail-fast] [--allow-failures] [--json <path>] [--show-failures N] [--auto-gc] [--auto-gc-threshold N] [--runtime-gc] [--runtime-gc-interval N] [--show-gc] [--expect-gc-baseline <path>] [--expect-collections-total-min N] [--expect-runtime-collections-min N] [--expect-runtime-ratio-min R] [--expect-reclaimed-objects-min N]"
    );
}

fn print_summary(summary: &SuiteSummary, show_gc: bool) {
    println!("test262 summary:");
    println!("  discovered: {}", summary.discovered);
    println!("  executed:   {}", summary.executed);
    println!("  skipped:    {}", summary.skipped);
    println!("  passed:     {}", summary.passed);
    println!("  failed:     {}", summary.failed);
    if show_gc {
        println!("gc summary:");
        println!("  collections_total: {}", summary.gc.collections_total);
        println!("  boundary_collections: {}", summary.gc.boundary_collections);
        println!("  runtime_collections: {}", summary.gc.runtime_collections);
        println!("  reclaimed_objects: {}", summary.gc.reclaimed_objects);
        println!("  mark_duration_ns: {}", summary.gc.mark_duration_ns);
        println!("  sweep_duration_ns: {}", summary.gc.sweep_duration_ns);
    }
}

fn write_summary_json(path: &PathBuf, summary: &SuiteSummary) -> Result<(), String> {
    let json = format!(
        "{{\n  \"discovered\": {},\n  \"executed\": {},\n  \"skipped\": {},\n  \"passed\": {},\n  \"failed\": {},\n  \"gc\": {{\n    \"collections_total\": {},\n    \"boundary_collections\": {},\n    \"runtime_collections\": {},\n    \"reclaimed_objects\": {},\n    \"mark_duration_ns\": {},\n    \"sweep_duration_ns\": {}\n  }}\n}}\n",
        summary.discovered,
        summary.executed,
        summary.skipped,
        summary.passed,
        summary.failed,
        summary.gc.collections_total,
        summary.gc.boundary_collections,
        summary.gc.runtime_collections,
        summary.gc.reclaimed_objects,
        summary.gc.mark_duration_ns,
        summary.gc.sweep_duration_ns
    );
    std::fs::write(path, json).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        GcExpectations, check_gc_expectations, merge_gc_expectations, parse_gc_expectations_str,
    };
    use test_harness::test262::SuiteSummary;

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
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("collections_total =="));
    }
}
