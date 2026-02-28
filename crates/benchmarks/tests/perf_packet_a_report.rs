use std::path::Path;

#[path = "../src/main.rs"]
#[allow(dead_code)]
mod benchmark_main;

fn mock_environment() -> benchmark_main::contract::EnvironmentInfo {
    benchmark_main::contract::EnvironmentInfo {
        os: "windows".to_string(),
        arch: "x86_64".to_string(),
        cpu_parallelism: 16,
        rustc: "rustc 1.90.0".to_string(),
        node: "v22.0.0".to_string(),
        quickjs_c: "unavailable".to_string(),
    }
}

#[test]
fn perf_packet_a_report_tags_packet_metadata_from_output_path() {
    let metadata = benchmark_main::build_perf_target_metadata(
        Path::new("target/benchmarks/engine-comparison.local-dev.packet-a.json"),
        &mock_environment(),
    );

    assert_eq!(
        metadata.optimization_mode,
        benchmark_main::contract::OptimizationMode::Packet
    );
    assert_eq!(metadata.optimization_tag, "packet-a");
    assert_eq!(metadata.packet_id.as_deref(), Some("packet-a"));
    assert!(metadata.same_host_required);
}

#[test]
fn perf_packet_a_report_enables_hotspot_by_default_for_packet_outputs() {
    let cli = match benchmark_main::contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-a.json".to_string(),
            "--allow-missing-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        benchmark_main::contract::CliParseResult::Run(cli) => cli,
        benchmark_main::contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        benchmark_main::infer_hotspot_attribution_default(&cli),
        "packet outputs must opt into hotspot attribution unless explicitly overridden"
    );
}
