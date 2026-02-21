#![forbid(unsafe_code)]

use std::path::PathBuf;
use test_harness::test262::{SuiteOptions, SuiteSummary, run_suite};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut root: Option<PathBuf> = None;
    let mut max_cases: Option<usize> = None;
    let mut fail_fast = false;
    let mut allow_failures = false;
    let mut json: Option<PathBuf> = None;

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

    let options = SuiteOptions {
        max_cases,
        fail_fast,
    };

    let summary = run_suite(&root, options).unwrap_or_else(|err| {
        panic!("suite run failed: {err}");
    });

    print_summary(&summary);

    if let Some(path) = json {
        write_summary_json(&path, &summary).unwrap_or_else(|err| {
            panic!("failed to write json summary to {}: {err}", path.display());
        });
    }

    if summary.failed > 0 && !allow_failures {
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "Usage: cargo run -p test-harness --bin test262-run -- --root <path> [--max-cases N] [--fail-fast] [--allow-failures] [--json <path>]"
    );
}

fn print_summary(summary: &SuiteSummary) {
    println!("test262 summary:");
    println!("  discovered: {}", summary.discovered);
    println!("  executed:   {}", summary.executed);
    println!("  skipped:    {}", summary.skipped);
    println!("  passed:     {}", summary.passed);
    println!("  failed:     {}", summary.failed);
}

fn write_summary_json(path: &PathBuf, summary: &SuiteSummary) -> Result<(), String> {
    let json = format!(
        "{{\n  \"discovered\": {},\n  \"executed\": {},\n  \"skipped\": {},\n  \"passed\": {},\n  \"failed\": {}\n}}\n",
        summary.discovered, summary.executed, summary.skipped, summary.passed, summary.failed
    );
    std::fs::write(path, json).map_err(|err| err.to_string())
}
