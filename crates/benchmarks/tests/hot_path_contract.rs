use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[path = "../src/contract.rs"]
#[allow(dead_code)]
mod contract;

fn minimal_report() -> contract::BenchmarkReport {
    contract::BenchmarkReport {
        schema_version: contract::SCHEMA_VERSION,
        generated_at_utc: "2026-02-28T00:00:00Z".to_string(),
        run_profile: contract::RunProfile::LocalDev,
        timing_mode: contract::TimingMode::EvalPerIteration,
        config: contract::BenchmarkConfig {
            iterations: 200,
            samples: 7,
            warmup_iterations: 3,
        },
        reproducibility: contract::ReproducibilityMetadata::for_run(
            contract::RunProfile::LocalDev,
            Path::new("target/benchmarks/engine-comparison.local-dev.phase11-baseline.json"),
        ),
        environment: contract::EnvironmentInfo {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            cpu_parallelism: 8,
            rustc: "rustc 1.87.0".to_string(),
            node: "v22.0.0".to_string(),
            quickjs_c: "unavailable".to_string(),
        },
        cases: vec![],
        aggregate: contract::AggregateReport {
            mean_ms_per_engine: BTreeMap::new(),
            relative_to_qjs_rs: BTreeMap::new(),
        },
        perf_target: contract::PerfTargetMetadata {
            policy_id: contract::PERF_TARGET_POLICY_ID,
            authoritative_run_profile: contract::PERF_TARGET_AUTHORITY_PROFILE,
            authoritative_timing_mode: contract::PERF_TARGET_AUTHORITY_TIMING_MODE,
            same_host_required: true,
            host_fingerprint: "host-a|linux|x86_64|8".to_string(),
            optimization_mode: contract::OptimizationMode::Baseline,
            optimization_tag: "phase11-baseline".to_string(),
            packet_id: None,
            required_comparators: contract::REQUIRED_CLOSURE_COMPARATORS.to_vec(),
            optional_comparators: contract::OPTIONAL_CLOSURE_COMPARATORS.to_vec(),
        },
        qjs_rs_hotspot_attribution: Some(contract::HotspotAttributionSnapshot {
            enabled: true,
            source: "vm-hotspot-attribution-v1",
            total: contract::HotspotAttributionCounters {
                numeric_ops: 10,
                identifier_resolution: 6,
                identifier_resolution_fallback_scans: 0,
                packet_d_slot_guard_hits: 0,
                packet_d_slot_guard_misses: 0,
                packet_d_slot_guard_revalidate_hits: 0,
                packet_d_slot_guard_revalidate_misses: 0,
                packet_g_name_guard_hits: 0,
                packet_g_name_guard_misses: 0,
                packet_g_name_guard_revalidate_hits: 0,
                packet_g_name_guard_revalidate_misses: 0,
                array_indexed_property_get: 2,
                array_indexed_property_set: 2,
            },
            per_case: BTreeMap::from([(
                "arith-loop".to_string(),
                contract::HotspotAttributionCounters {
                    numeric_ops: 10,
                    identifier_resolution: 6,
                    identifier_resolution_fallback_scans: 0,
                    packet_d_slot_guard_hits: 0,
                    packet_d_slot_guard_misses: 0,
                    packet_d_slot_guard_revalidate_hits: 0,
                    packet_d_slot_guard_revalidate_misses: 0,
                    packet_g_name_guard_hits: 0,
                    packet_g_name_guard_misses: 0,
                    packet_g_name_guard_revalidate_hits: 0,
                    packet_g_name_guard_revalidate_misses: 0,
                    array_indexed_property_get: 2,
                    array_indexed_property_set: 2,
                },
            )]),
        }),
    }
}

#[test]
fn hot_path_contract_serializes_perf_target_and_hotspot_attribution() {
    let report = minimal_report();
    let payload: Value = serde_json::to_value(report).expect("report should serialize");

    assert_eq!(
        payload
            .get("perf_target")
            .and_then(|value| value.get("policy_id"))
            .and_then(Value::as_str),
        Some("phase11-perf03-local-dev-eval-per-iteration")
    );
    assert_eq!(
        payload
            .get("perf_target")
            .and_then(|value| value.get("optimization_mode"))
            .and_then(Value::as_str),
        Some("baseline")
    );
    assert_eq!(
        payload
            .get("qjs_rs_hotspot_attribution")
            .and_then(|value| value.get("total"))
            .and_then(|value| value.get("numeric_ops"))
            .and_then(Value::as_u64),
        Some(10)
    );
}

#[test]
fn hot_path_contract_infers_optimization_descriptor_from_output_path() {
    let baseline = contract::infer_optimization_descriptor(Path::new(
        "target/benchmarks/engine-comparison.local-dev.phase11-baseline.json",
    ));
    assert_eq!(baseline.mode, contract::OptimizationMode::Baseline);
    assert_eq!(baseline.tag, "phase11-baseline");
    assert_eq!(baseline.packet_id, None);

    let packet = contract::infer_optimization_descriptor(Path::new(
        "target/benchmarks/engine-comparison.local-dev.packet-a.json",
    ));
    assert_eq!(packet.mode, contract::OptimizationMode::Packet);
    assert_eq!(packet.tag, "packet-a");
    assert_eq!(packet.packet_id.as_deref(), Some("packet-a"));
}
