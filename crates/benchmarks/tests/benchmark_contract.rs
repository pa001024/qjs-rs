use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[path = "../src/contract.rs"]
#[allow(dead_code)]
mod contract;

#[test]
fn benchmark_contract_required_case_ids() {
    let actual: Vec<&str> = contract::required_case_catalog()
        .iter()
        .map(|case| case.id)
        .collect();

    assert_eq!(
        actual,
        vec!["arith-loop", "fib-iterative", "array-sum", "json-roundtrip"]
    );
}

#[test]
fn benchmark_report_contract_envelope_fields() {
    let report = contract::BenchmarkReport {
        schema_version: contract::SCHEMA_VERSION,
        generated_at_utc: "2026-02-27T00:00:00Z".to_string(),
        run_profile: contract::RunProfile::LocalDev,
        timing_mode: contract::TimingMode::EvalPerIteration,
        config: contract::BenchmarkConfig {
            iterations: 200,
            samples: 7,
            warmup_iterations: 3,
        },
        reproducibility: contract::ReproducibilityMetadata::for_run(
            contract::RunProfile::LocalDev,
            Path::new("target/benchmarks/engine-comparison.local-dev.json"),
        ),
        environment: contract::EnvironmentInfo {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            cpu_parallelism: 8,
            rustc: "rustc 1.85.0".to_string(),
            node: "v20.0.0".to_string(),
            quickjs_c: "QuickJS version".to_string(),
        },
        cases: vec![contract::CaseReport {
            id: "arith-loop".to_string(),
            title: "Arithmetic Loop".to_string(),
            description: "integer arithmetic in a hot for-loop".to_string(),
            engines: BTreeMap::new(),
        }],
        aggregate: contract::AggregateReport {
            mean_ms_per_engine: BTreeMap::new(),
            relative_to_qjs_rs: BTreeMap::new(),
        },
    };

    let serialized: Value = serde_json::to_value(report).expect("report should serialize");

    assert_eq!(serialized["schema_version"], "bench.v1");
    assert_eq!(serialized["run_profile"], "local-dev");
    assert_eq!(serialized["timing_mode"], "eval-per-iteration");

    for field in [
        "schema_version",
        "generated_at_utc",
        "run_profile",
        "timing_mode",
        "config",
        "reproducibility",
        "environment",
        "cases",
        "aggregate",
    ] {
        assert!(
            serialized.get(field).is_some(),
            "missing required report field: {field}"
        );
    }

    let config = serialized
        .get("config")
        .and_then(Value::as_object)
        .expect("config must be object");
    for field in ["iterations", "samples", "warmup_iterations"] {
        assert!(config.contains_key(field), "missing config field: {field}");
    }

    let reproducibility = serialized
        .get("reproducibility")
        .and_then(Value::as_object)
        .expect("reproducibility must be object");
    for field in [
        "required_engines",
        "required_case_ids",
        "output_policy",
        "engine_status",
    ] {
        assert!(
            reproducibility.contains_key(field),
            "missing reproducibility field: {field}"
        );
    }

    let required_engines = reproducibility["required_engines"]
        .as_array()
        .expect("required_engines must be array")
        .iter()
        .map(|value| value.as_str().expect("engine id should be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        required_engines,
        vec!["qjs-rs", "boa-engine", "nodejs", "quickjs-c"]
    );

    let required_case_ids = reproducibility["required_case_ids"]
        .as_array()
        .expect("required_case_ids must be array")
        .iter()
        .map(|value| value.as_str().expect("case id should be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        required_case_ids,
        vec!["arith-loop", "fib-iterative", "array-sum", "json-roundtrip"]
    );
}
