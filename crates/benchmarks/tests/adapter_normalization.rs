use std::path::Path;

#[path = "../src/contract.rs"]
#[allow(dead_code)]
mod contract;

#[test]
fn adapter_normalization_timing_mode_contract_is_shared() {
    let mode = contract::TimingMode::EvalPerIteration;
    for engine in contract::EngineKind::all_required() {
        assert_eq!(
            contract::test_support::timing_mode_for_engine(engine, mode),
            mode
        );
    }
}

#[test]
fn adapter_normalization_command_resolution_honors_precedence() {
    let parsed = contract::parse_cli_args_with_env(
        vec![
            "--node-command".to_string(),
            "node-from-cli".to_string(),
            "--node-path".to_string(),
            "cli-node-path".to_string(),
            "--node-workdir".to_string(),
            "cli-node-workdir".to_string(),
            "--quickjs-command".to_string(),
            "qjs-from-cli".to_string(),
            "--quickjs-path".to_string(),
            "cli-qjs-path".to_string(),
            "--quickjs-workdir".to_string(),
            "cli-qjs-workdir".to_string(),
            "--allow-missing-comparators".to_string(),
        ],
        &[
            (contract::ENV_NODE_COMMAND, "node-from-env"),
            (contract::ENV_NODE_PATH, "env-node-path"),
            (contract::ENV_NODE_WORKDIR, "env-node-workdir"),
            (contract::ENV_QUICKJS_COMMAND, "qjs-from-env"),
            (contract::ENV_QUICKJS_PATH, "env-qjs-path"),
            (contract::ENV_QUICKJS_WORKDIR, "env-qjs-workdir"),
            (contract::ENV_STRICT_COMPARATORS, "true"),
        ],
    )
    .expect("cli args should parse");

    let cli = match parsed {
        contract::CliParseResult::Run(cli) => cli,
        contract::CliParseResult::Help => panic!("expected run args"),
    };

    assert_eq!(cli.comparators.node.command, "node-from-cli");
    assert_eq!(
        cli.comparators
            .node
            .path
            .as_ref()
            .and_then(|path| path.to_str()),
        Some("cli-node-path")
    );
    assert_eq!(
        cli.comparators
            .node
            .workdir
            .as_ref()
            .and_then(|path| path.to_str()),
        Some("cli-node-workdir")
    );
    assert_eq!(cli.comparators.quickjs.command, "qjs-from-cli");
    assert_eq!(
        cli.comparators
            .quickjs
            .path
            .as_ref()
            .and_then(|path| path.to_str()),
        Some("cli-qjs-path")
    );
    assert_eq!(
        cli.comparators
            .quickjs
            .workdir
            .as_ref()
            .and_then(|path| path.to_str()),
        Some("cli-qjs-workdir")
    );
    assert!(!cli.comparators.strict_external);
}

#[test]
fn adapter_normalization_reproducibility_metadata_is_complete() {
    let metadata = contract::ReproducibilityMetadata::for_run_with_engine_status(
        contract::RunProfile::LocalDev,
        Path::new("target/benchmarks/engine-comparison.local-dev.json"),
        false,
        vec![
            contract::EngineExecutionMetadata {
                engine: "qjs-rs".to_string(),
                status: contract::EngineAvailabilityStatus::Available,
                command: "in-process".to_string(),
                path: None,
                workdir: None,
                version: Some("qjs-rs test".to_string()),
                reason: None,
            },
            contract::EngineExecutionMetadata {
                engine: "nodejs".to_string(),
                status: contract::EngineAvailabilityStatus::Available,
                command: "node".to_string(),
                path: Some("/usr/bin/node".to_string()),
                workdir: Some("/tmp".to_string()),
                version: Some("v22.0.0".to_string()),
                reason: None,
            },
        ],
    );

    let json = serde_json::to_value(metadata).expect("metadata should serialize");
    let engine_status = json["engine_status"]
        .as_array()
        .expect("engine_status array");
    let node = engine_status
        .iter()
        .find(|entry| entry["engine"] == "nodejs")
        .expect("node entry present");

    assert_eq!(json["comparator_strict_mode"], false);
    assert_eq!(node["status"], "available");
    assert_eq!(node["command"], "node");
    assert_eq!(node["path"], "/usr/bin/node");
    assert_eq!(node["workdir"], "/tmp");
    assert_eq!(node["version"], "v22.0.0");
}

#[test]
fn adapter_normalization_checksum_fold_is_value_based() {
    let checksum = contract::test_support::guard_delta_from_number_or_bool(Some(10.0), Some(false))
        + contract::test_support::guard_delta_from_number_or_bool(None, Some(true))
        + contract::test_support::guard_delta_from_number_or_bool(None, Some(false));

    assert_eq!(checksum, 11.0);
    assert_ne!(checksum, 3.0);
}
