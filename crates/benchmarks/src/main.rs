#![forbid(unsafe_code)]

pub(crate) mod contract;

use anyhow::{Context as _, Result, anyhow, bail};
use boa_engine::{Context as BoaContext, Source};
use bytecode::compile_script;
use contract::{
    AggregateReport, BenchmarkReport, CaseEngineResult, CaseReport, CliParseResult,
    ComparatorConfig, ComparatorTarget, EngineAvailabilityStatus, EngineExecutionMetadata,
    EngineKind, EnvironmentInfo, GUARD_CHECKSUM_MODE, HotspotAttributionCounters,
    HotspotAttributionSnapshot, PerfTargetMetadata, ReproducibilityMetadata,
    RequiredCaseDefinition, SCHEMA_VERSION, TimingMode, help_text, parse_cli_args,
    required_case_catalog, test_support,
};
use parser::parse_script;
use runtime::JsValue;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use vm::{Vm, perf::HotspotAttribution};

#[derive(Debug, Clone)]
struct BenchCase {
    definition: &'static RequiredCaseDefinition,
    script_body: &'static str,
}

fn benchmark_cases() -> Vec<BenchCase> {
    required_case_catalog()
        .iter()
        .map(|definition| BenchCase {
            definition,
            script_body: script_body_for_case(definition.id),
        })
        .collect()
}

fn script_body_for_case(case_id: &str) -> &'static str {
    match case_id {
        "arith-loop" => {
            r#"
let acc = 0;
for (let i = 0; i < 4000; i = i + 1) {
  acc = acc + i * 3 - i;
}
return acc;
"#
        }
        "fib-iterative" => {
            r#"
function fib(n) {
  let a = 0;
  let b = 1;
  for (let i = 0; i < n; i = i + 1) {
    let t = a + b;
    a = b;
    b = t;
  }
  return a;
}
return fib(220);
"#
        }
        "array-sum" => {
            r#"
let arr = [];
for (let i = 0; i < 2000; i = i + 1) {
  arr[i] = i;
}
let sum = 0;
for (let i = 0; i < arr.length; i = i + 1) {
  sum = sum + arr[i];
}
return sum;
"#
        }
        "json-roundtrip" => {
            r#"
let value = {
  alpha: 1,
  beta: [1, 2, 3, 4],
  gamma: { x: "hello", y: 42 }
};
let text = JSON.stringify(value);
let parsed = JSON.parse(text);
return parsed.gamma.y + parsed.beta[1];
"#
        }
        unknown => panic!("contract case id without script body: {unknown}"),
    }
}

fn wrap_script(script_body: &str) -> String {
    format!("(() => {{ {script_body} }})()")
}

fn extract_number(value: &JsValue) -> f64 {
    match value {
        JsValue::Number(n) => *n,
        JsValue::Bool(true) => 1.0,
        JsValue::Bool(false) => 0.0,
        _ => 0.0,
    }
}

pub(crate) fn guard_delta_from_number_or_bool(number: Option<f64>, boolean: Option<bool>) -> f64 {
    test_support::guard_delta_from_number_or_bool(number, boolean)
}

pub(crate) fn timing_mode_for_engine(
    engine: EngineKind,
    run_timing_mode: TimingMode,
) -> TimingMode {
    test_support::timing_mode_for_engine(engine, run_timing_mode)
}

#[derive(Debug, Clone, Copy)]
struct SampleMeasurement {
    elapsed_ms: f64,
    guard_checksum: f64,
    hotspot_attribution: Option<HotspotAttribution>,
}

#[derive(Debug, Clone, Copy)]
struct QjsRsEvalConfig {
    hotspot_attribution_enabled: bool,
    packet_c_enabled: bool,
    packet_d_enabled: bool,
    packet_g_enabled: bool,
    packet_h_enabled: bool,
    packet_i_enabled: bool,
}

fn run_qjs_rs_eval_per_iteration(
    script: &str,
    iterations: usize,
    config: QjsRsEvalConfig,
) -> Result<SampleMeasurement> {
    let realm = runtime::Realm::default();
    let mut vm = Vm::with_perf_from_env();
    vm.set_hotspot_attribution_enabled(false);
    vm.set_packet_c_fast_path_enabled(config.packet_c_enabled);
    vm.set_packet_d_fast_path_enabled(config.packet_d_enabled);
    vm.set_packet_d_fast_path_metrics_enabled(false);
    vm.set_packet_g_fast_path_enabled(config.packet_g_enabled);
    vm.set_packet_g_fast_path_metrics_enabled(false);
    vm.set_packet_h_fast_path_enabled(config.packet_h_enabled);
    vm.set_packet_h_fast_path_metrics_enabled(false);
    vm.set_packet_i_revalidate_enabled(config.packet_i_enabled);

    let parsed = parse_script(script).map_err(|e| anyhow!("qjs-rs parse error: {}", e.message))?;
    let chunk = compile_script(&parsed);
    let start = Instant::now();
    let mut checksum = 0.0;
    for _ in 0..iterations {
        let value = vm
            .execute_in_realm_persistent(&chunk, &realm)
            .map_err(|e| anyhow!("qjs-rs execute error: {e:?}"))?;
        checksum += extract_number(&value);
    }
    let hotspot_attribution = if config.hotspot_attribution_enabled {
        vm.set_packet_d_fast_path_metrics_enabled(config.packet_d_enabled);
        vm.set_packet_g_fast_path_metrics_enabled(config.packet_g_enabled);
        vm.set_packet_h_fast_path_metrics_enabled(config.packet_h_enabled);
        vm.set_hotspot_attribution_enabled(true);
        vm.reset_hotspot_attribution();
        let _ = vm
            .execute_in_realm_persistent(&chunk, &realm)
            .map_err(|e| anyhow!("qjs-rs execute error: {e:?}"))?;
        vm.set_packet_d_fast_path_metrics_enabled(false);
        vm.set_packet_g_fast_path_metrics_enabled(false);
        vm.set_packet_h_fast_path_metrics_enabled(false);
        vm.hotspot_attribution_snapshot()
    } else {
        None
    };
    std::hint::black_box(checksum);
    Ok(SampleMeasurement {
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        guard_checksum: checksum,
        hotspot_attribution,
    })
}

fn run_boa_engine_eval_per_iteration(script: &str, iterations: usize) -> Result<SampleMeasurement> {
    let mut context = BoaContext::default();
    let start = Instant::now();
    let mut checksum = 0.0;
    for _ in 0..iterations {
        let value = context
            .eval(Source::from_bytes(script.as_bytes()))
            .map_err(|e| anyhow!("boa-engine error: {e}"))?;
        let normalized = guard_delta_from_number_or_bool(value.as_number(), value.as_boolean());
        checksum += normalized;
    }
    std::hint::black_box(checksum);
    Ok(SampleMeasurement {
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        guard_checksum: checksum,
        hotspot_attribution: None,
    })
}

#[derive(Debug, Deserialize)]
struct NodeResult {
    elapsed_ms: f64,
    guard: f64,
}

#[derive(Debug, Serialize)]
struct NodePayload<'a> {
    script: &'a str,
    iterations: usize,
}

fn run_nodejs_eval_per_iteration(
    script: &str,
    iterations: usize,
    comparator: &ComparatorTarget,
) -> Result<SampleMeasurement> {
    let payload = serde_json::to_string(&NodePayload { script, iterations })?;
    let node_snippet = r#"
const payload = JSON.parse(process.argv[1]);
const code = payload.script;
const iterations = payload.iterations;
let guard = 0;
const start = process.hrtime.bigint();
for (let i = 0; i < iterations; i += 1) {
  const value = eval(code);
  if (typeof value === "number") {
    guard += value;
  } else if (value === true) {
    guard += 1;
  }
}
const end = process.hrtime.bigint();
process.stdout.write(JSON.stringify({
  elapsed_ms: Number(end - start) / 1e6,
  guard,
}));
"#;
    let mut command = Command::new(comparator.executable());
    if let Some(workdir) = &comparator.workdir {
        command.current_dir(workdir);
    }
    let output = command
        .arg("-e")
        .arg(node_snippet)
        .arg(payload)
        .output()
        .context("failed to execute nodejs benchmark")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("nodejs benchmark failed: {stderr}");
    }
    let result: NodeResult = serde_json::from_slice(&output.stdout)
        .context("failed to parse nodejs benchmark output (expected json with elapsed_ms field)")?;
    Ok(SampleMeasurement {
        elapsed_ms: result.elapsed_ms,
        guard_checksum: result.guard,
        hotspot_attribution: None,
    })
}

fn run_quickjs_c_eval_per_iteration(
    script: &str,
    iterations: usize,
    comparator: &ComparatorTarget,
) -> Result<SampleMeasurement> {
    let script_json = serde_json::to_string(script)?;
    let quickjs_code = format!(
        "const code = {script_json};\n\
         const iterations = {iterations};\n\
         let guard = 0;\n\
         const start = Date.now() >>> 0;\n\
         for (let i = 0; i < iterations; i++) {{\n\
           const value = eval(code);\n\
           if (typeof value === 'number') guard += value;\n\
           else if (value === true) guard += 1;\n\
         }}\n\
         const end = Date.now() >>> 0;\n\
         const elapsed_ms = (end - start) >>> 0;\n\
         console.log(JSON.stringify({{ elapsed_ms, guard }}));"
    );

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let quickjs_script_path = env::temp_dir().join(format!(
        "qjs-rs-benchmark-{}-{timestamp_ms}.js",
        std::process::id()
    ));
    fs::write(&quickjs_script_path, quickjs_code.as_bytes()).with_context(|| {
        format!(
            "failed to write temporary quickjs benchmark script: {}",
            quickjs_script_path.display()
        )
    })?;

    let mut command = Command::new(comparator.executable());
    if let Some(workdir) = &comparator.workdir {
        command.current_dir(workdir);
    }
    let output = command
        .arg(quickjs_script_path.as_os_str())
        .output()
        .context("failed to execute quickjs-c benchmark")?;
    let _ = fs::remove_file(&quickjs_script_path);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!("quickjs-c benchmark failed: stdout={stdout}; stderr={stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.contains("elapsed_ms"))
        .ok_or_else(|| anyhow!("quickjs-c output missing elapsed_ms json: {stdout}"))?;
    let result: NodeResult = serde_json::from_str(line)?;
    Ok(SampleMeasurement {
        elapsed_ms: result.elapsed_ms,
        guard_checksum: result.guard,
        hotspot_attribution: None,
    })
}

#[derive(Debug, Clone, Copy)]
struct RunEngineCaseContext<'a> {
    timing_mode: TimingMode,
    script: &'a str,
    iterations: usize,
    comparators: &'a ComparatorConfig,
    hotspot_attribution_enabled: bool,
    packet_c_enabled: bool,
    packet_d_enabled: bool,
    packet_g_enabled: bool,
    packet_h_enabled: bool,
    packet_i_enabled: bool,
}

fn run_engine_case(
    engine: EngineKind,
    context: RunEngineCaseContext<'_>,
) -> Result<SampleMeasurement> {
    let adapter_mode = timing_mode_for_engine(engine, context.timing_mode);
    match adapter_mode {
        TimingMode::EvalPerIteration => match engine {
            EngineKind::QjsRs => run_qjs_rs_eval_per_iteration(
                context.script,
                context.iterations,
                QjsRsEvalConfig {
                    hotspot_attribution_enabled: context.hotspot_attribution_enabled,
                    packet_c_enabled: context.packet_c_enabled,
                    packet_d_enabled: context.packet_d_enabled,
                    packet_g_enabled: context.packet_g_enabled,
                    packet_h_enabled: context.packet_h_enabled,
                    packet_i_enabled: context.packet_i_enabled,
                },
            ),
            EngineKind::BoaEngine => {
                run_boa_engine_eval_per_iteration(context.script, context.iterations)
            }
            EngineKind::NodeJs => run_nodejs_eval_per_iteration(
                context.script,
                context.iterations,
                &context.comparators.node,
            ),
            EngineKind::QuickJsC => run_quickjs_c_eval_per_iteration(
                context.script,
                context.iterations,
                &context.comparators.quickjs,
            ),
        },
    }
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let len = sorted.len();
    if len % 2 == 1 {
        sorted[len / 2]
    } else {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    }
}

fn stddev(values: &[f64], mean: f64) -> f64 {
    let variance = values
        .iter()
        .map(|value| {
            let diff = *value - mean;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn summarize(
    sample_measurements: Vec<SampleMeasurement>,
    iterations: usize,
    warmup_guard_checksum: f64,
) -> CaseEngineResult {
    let sample_ms: Vec<f64> = sample_measurements
        .iter()
        .map(|sample| sample.elapsed_ms)
        .collect();
    let sample_guard_checksums: Vec<f64> = sample_measurements
        .iter()
        .map(|sample| sample.guard_checksum)
        .collect();

    let mean_ms = sample_ms.iter().sum::<f64>() / sample_ms.len() as f64;
    let median_ms = median(&sample_ms);
    let min_ms = sample_ms.iter().copied().fold(f64::INFINITY, f64::min);
    let max_ms = sample_ms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let stddev_ms = stddev(&sample_ms, mean_ms);
    let throughput_ops_per_sec = (iterations as f64) / (mean_ms / 1000.0);
    let mean_guard_checksum =
        sample_guard_checksums.iter().sum::<f64>() / sample_guard_checksums.len() as f64;
    let guard_checksum_consistent = sample_guard_checksums.windows(2).all(|pair| {
        let left = pair[0];
        let right = pair[1];
        (left - right).abs() <= 0.000_001
    });
    CaseEngineResult {
        sample_ms,
        mean_ms,
        median_ms,
        min_ms,
        max_ms,
        stddev_ms,
        throughput_ops_per_sec,
        guard_checksum_mode: GUARD_CHECKSUM_MODE,
        warmup_guard_checksum,
        sample_guard_checksums,
        mean_guard_checksum,
        guard_checksum_consistent,
    }
}

fn preflight_engine_execution(
    comparators: &ComparatorConfig,
) -> Result<Vec<EngineExecutionMetadata>> {
    let mut metadata = vec![
        EngineExecutionMetadata {
            engine: EngineKind::QjsRs.as_str().to_string(),
            status: EngineAvailabilityStatus::Available,
            command: "in-process".to_string(),
            path: None,
            workdir: None,
            version: Some(format!("qjs-rs {}", env!("CARGO_PKG_VERSION"))),
            reason: None,
        },
        EngineExecutionMetadata {
            engine: EngineKind::BoaEngine.as_str().to_string(),
            status: EngineAvailabilityStatus::Available,
            command: "in-process".to_string(),
            path: None,
            workdir: None,
            version: Some("boa-engine (in-process)".to_string()),
            reason: None,
        },
    ];

    metadata.push(preflight_external_engine(
        EngineKind::NodeJs,
        &comparators.node,
        &["--version"],
    ));
    metadata.push(preflight_external_engine(
        EngineKind::QuickJsC,
        &comparators.quickjs,
        &["--version", "-v", "-h"],
    ));

    if comparators.strict_external {
        let missing = metadata
            .iter()
            .filter(|entry| {
                matches!(entry.engine.as_str(), "nodejs" | "quickjs-c")
                    && entry.status != EngineAvailabilityStatus::Available
            })
            .map(|entry| {
                format!(
                    "{} ({})",
                    entry.engine,
                    entry
                        .reason
                        .as_deref()
                        .unwrap_or("comparator preflight failed")
                )
            })
            .collect::<Vec<_>>();

        if !missing.is_empty() {
            bail!(
                "strict comparator preflight failed: {}. Configure comparators with --node-path/--quickjs-path (or env BENCH_* overrides) before rerunning.",
                missing.join("; ")
            );
        }
    }

    Ok(metadata)
}

fn preflight_external_engine(
    engine: EngineKind,
    comparator: &ComparatorTarget,
    version_args: &[&str],
) -> EngineExecutionMetadata {
    let executable = comparator.executable();
    let resolved_path = resolve_executable_path(comparator);
    let workdir = comparator
        .workdir
        .as_ref()
        .map(|dir| dir.display().to_string());
    let mut last_reason: Option<String> = None;

    for version_arg in version_args {
        let mut command = Command::new(&executable);
        if let Some(workdir) = &comparator.workdir {
            command.current_dir(workdir);
        }
        command.arg(version_arg);
        match command.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if output.status.success() {
                    let version = first_non_empty_line(&stdout, &stderr)
                        .unwrap_or_else(|| "version probe succeeded".to_string());
                    return EngineExecutionMetadata {
                        engine: engine.as_str().to_string(),
                        status: EngineAvailabilityStatus::Available,
                        command: comparator.command.clone(),
                        path: resolved_path.clone(),
                        workdir: workdir.clone(),
                        version: Some(version),
                        reason: None,
                    };
                }

                if let Some(version) = first_non_empty_line(&stdout, &stderr) {
                    if version.to_ascii_lowercase().contains("quickjs")
                        || version.to_ascii_lowercase().contains("node")
                    {
                        return EngineExecutionMetadata {
                            engine: engine.as_str().to_string(),
                            status: EngineAvailabilityStatus::Available,
                            command: comparator.command.clone(),
                            path: resolved_path.clone(),
                            workdir: workdir.clone(),
                            version: Some(version),
                            reason: None,
                        };
                    }
                }

                last_reason = Some(format!(
                    "probe `{}` exited with status {}",
                    version_arg,
                    output
                        .status
                        .code()
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ));
            }
            Err(error) => {
                let status = if error.kind() == std::io::ErrorKind::NotFound {
                    EngineAvailabilityStatus::Missing
                } else {
                    EngineAvailabilityStatus::Unsupported
                };
                return EngineExecutionMetadata {
                    engine: engine.as_str().to_string(),
                    status,
                    command: comparator.command.clone(),
                    path: resolved_path,
                    workdir,
                    version: None,
                    reason: Some(error.to_string()),
                };
            }
        }
    }

    EngineExecutionMetadata {
        engine: engine.as_str().to_string(),
        status: EngineAvailabilityStatus::Unsupported,
        command: comparator.command.clone(),
        path: resolved_path,
        workdir,
        version: None,
        reason: last_reason.or_else(|| Some("no successful version probe".to_string())),
    }
}

fn first_non_empty_line(primary: &str, fallback: &str) -> Option<String> {
    primary
        .lines()
        .find(|line| !line.trim().is_empty())
        .or_else(|| fallback.lines().find(|line| !line.trim().is_empty()))
        .map(|line| line.trim().to_string())
}

fn resolve_executable_path(comparator: &ComparatorTarget) -> Option<String> {
    if let Some(path) = &comparator.path {
        return Some(path.display().to_string());
    }

    let lookup_program = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    Command::new(lookup_program)
        .arg(&comparator.command)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
        })
}

fn engine_available(engine: EngineKind, metadata: &[EngineExecutionMetadata]) -> bool {
    metadata
        .iter()
        .find(|entry| entry.engine == engine.as_str())
        .map(|entry| entry.status == EngineAvailabilityStatus::Available)
        .unwrap_or(false)
}

fn collect_environment(engine_metadata: &[EngineExecutionMetadata]) -> EnvironmentInfo {
    let rustc = Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let node = engine_metadata
        .iter()
        .find(|entry| entry.engine == EngineKind::NodeJs.as_str())
        .and_then(|entry| entry.version.clone())
        .unwrap_or_else(|| "unavailable".to_string());
    let quickjs_c = engine_metadata
        .iter()
        .find(|entry| entry.engine == EngineKind::QuickJsC.as_str())
        .and_then(|entry| entry.version.clone())
        .unwrap_or_else(|| "unavailable".to_string());
    EnvironmentInfo {
        os: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        cpu_parallelism: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        rustc,
        node,
        quickjs_c,
    }
}

pub(crate) fn build_perf_target_metadata(
    output: &Path,
    environment: &EnvironmentInfo,
) -> PerfTargetMetadata {
    test_support::build_perf_target_metadata(output, environment)
}

pub(crate) fn infer_hotspot_attribution_default(cli: &contract::CliArgs) -> bool {
    test_support::infer_hotspot_attribution_default(cli)
}

pub(crate) fn infer_packet_c_enabled(cli: &contract::CliArgs) -> bool {
    let descriptor = contract::infer_optimization_descriptor(&cli.output);
    descriptor.packet_id.as_deref().is_some_and(|packet_id| {
        packet_id.starts_with("packet-c")
            || packet_id.starts_with("packet-d")
            || packet_id.starts_with("packet-e")
            || packet_id.starts_with("packet-f")
            || packet_id.starts_with("packet-g")
            || packet_id.starts_with("packet-h")
            || packet_id.starts_with("packet-i")
            || packet_id.starts_with("packet-final")
    }) || matches!(cli.run_profile, contract::RunProfile::LocalDev)
}

pub(crate) fn infer_packet_d_enabled(cli: &contract::CliArgs) -> bool {
    let descriptor = contract::infer_optimization_descriptor(&cli.output);
    descriptor.packet_id.as_deref().is_some_and(|packet_id| {
        packet_id.starts_with("packet-d")
            || packet_id.starts_with("packet-e")
            || packet_id.starts_with("packet-f")
            || packet_id.starts_with("packet-g")
            || packet_id.starts_with("packet-h")
            || packet_id.starts_with("packet-i")
            || packet_id.starts_with("packet-final")
    }) || matches!(cli.run_profile, contract::RunProfile::LocalDev)
}

pub(crate) fn infer_packet_g_enabled(cli: &contract::CliArgs) -> bool {
    let descriptor = contract::infer_optimization_descriptor(&cli.output);
    descriptor.packet_id.as_deref().is_some_and(|packet_id| {
        packet_id.starts_with("packet-g")
            || packet_id.starts_with("packet-h")
            || packet_id.starts_with("packet-i")
    })
}

pub(crate) fn infer_packet_h_enabled(cli: &contract::CliArgs) -> bool {
    let descriptor = contract::infer_optimization_descriptor(&cli.output);
    descriptor.packet_id.as_deref().is_some_and(|packet_id| {
        packet_id.starts_with("packet-h") || packet_id.starts_with("packet-i")
    })
}

pub(crate) fn infer_packet_i_enabled(cli: &contract::CliArgs) -> bool {
    let descriptor = contract::infer_optimization_descriptor(&cli.output);
    descriptor
        .packet_id
        .as_deref()
        .is_some_and(|packet_id| packet_id.starts_with("packet-i"))
}

fn to_hotspot_counters(value: HotspotAttribution) -> HotspotAttributionCounters {
    HotspotAttributionCounters {
        numeric_ops: value.numeric_ops,
        identifier_resolution: value.identifier_resolution,
        identifier_resolution_fallback_scans: value.identifier_resolution_fallback_scans,
        packet_d_slot_guard_hits: value.packet_d_slot_guard_hits,
        packet_d_slot_guard_misses: value.packet_d_slot_guard_misses,
        packet_d_slot_guard_revalidate_hits: value.packet_d_slot_guard_revalidate_hits,
        packet_d_slot_guard_revalidate_misses: value.packet_d_slot_guard_revalidate_misses,
        packet_g_name_guard_hits: value.packet_g_name_guard_hits,
        packet_g_name_guard_misses: value.packet_g_name_guard_misses,
        packet_g_name_guard_revalidate_hits: value.packet_g_name_guard_revalidate_hits,
        packet_g_name_guard_revalidate_misses: value.packet_g_name_guard_revalidate_misses,
        packet_h_lexical_slot_guard_hits: value.packet_h_lexical_slot_guard_hits,
        packet_h_lexical_slot_guard_misses: value.packet_h_lexical_slot_guard_misses,
        array_indexed_property_get: value.array_indexed_property_get,
        array_indexed_property_set: value.array_indexed_property_set,
    }
}

fn merge_hotspot_counters(
    target: &mut HotspotAttributionCounters,
    source: &HotspotAttributionCounters,
) {
    target.numeric_ops = target.numeric_ops.saturating_add(source.numeric_ops);
    target.identifier_resolution = target
        .identifier_resolution
        .saturating_add(source.identifier_resolution);
    target.identifier_resolution_fallback_scans = target
        .identifier_resolution_fallback_scans
        .saturating_add(source.identifier_resolution_fallback_scans);
    target.packet_d_slot_guard_hits = target
        .packet_d_slot_guard_hits
        .saturating_add(source.packet_d_slot_guard_hits);
    target.packet_d_slot_guard_misses = target
        .packet_d_slot_guard_misses
        .saturating_add(source.packet_d_slot_guard_misses);
    target.packet_d_slot_guard_revalidate_hits = target
        .packet_d_slot_guard_revalidate_hits
        .saturating_add(source.packet_d_slot_guard_revalidate_hits);
    target.packet_d_slot_guard_revalidate_misses = target
        .packet_d_slot_guard_revalidate_misses
        .saturating_add(source.packet_d_slot_guard_revalidate_misses);
    target.packet_g_name_guard_hits = target
        .packet_g_name_guard_hits
        .saturating_add(source.packet_g_name_guard_hits);
    target.packet_g_name_guard_misses = target
        .packet_g_name_guard_misses
        .saturating_add(source.packet_g_name_guard_misses);
    target.packet_g_name_guard_revalidate_hits = target
        .packet_g_name_guard_revalidate_hits
        .saturating_add(source.packet_g_name_guard_revalidate_hits);
    target.packet_g_name_guard_revalidate_misses = target
        .packet_g_name_guard_revalidate_misses
        .saturating_add(source.packet_g_name_guard_revalidate_misses);
    target.packet_h_lexical_slot_guard_hits = target
        .packet_h_lexical_slot_guard_hits
        .saturating_add(source.packet_h_lexical_slot_guard_hits);
    target.packet_h_lexical_slot_guard_misses = target
        .packet_h_lexical_slot_guard_misses
        .saturating_add(source.packet_h_lexical_slot_guard_misses);
    target.array_indexed_property_get = target
        .array_indexed_property_get
        .saturating_add(source.array_indexed_property_get);
    target.array_indexed_property_set = target
        .array_indexed_property_set
        .saturating_add(source.array_indexed_property_set);
}

fn aggregate_case_hotspot_attribution(
    samples: &[SampleMeasurement],
) -> Option<HotspotAttributionCounters> {
    let mut aggregate = HotspotAttribution::default();
    let mut found = false;
    for sample in samples {
        if let Some(snapshot) = sample.hotspot_attribution {
            aggregate.merge(snapshot);
            found = true;
        }
    }
    found.then_some(to_hotspot_counters(aggregate))
}

fn aggregate(cases: &[CaseReport]) -> AggregateReport {
    let mut totals: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for case in cases {
        for (engine, result) in &case.engines {
            totals
                .entry(engine.clone())
                .or_default()
                .push(result.mean_ms);
        }
    }

    let mean_ms_per_engine = totals
        .iter()
        .map(|(engine, values)| {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            (engine.clone(), mean)
        })
        .collect::<BTreeMap<_, _>>();

    let qjs_baseline = mean_ms_per_engine
        .get("qjs-rs")
        .copied()
        .unwrap_or(1.0)
        .max(0.000_001);
    let relative_to_qjs_rs = mean_ms_per_engine
        .iter()
        .map(|(engine, mean)| (engine.clone(), qjs_baseline / *mean))
        .collect::<BTreeMap<_, _>>();

    AggregateReport {
        mean_ms_per_engine,
        relative_to_qjs_rs,
    }
}

fn ensure_output_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn generated_at_utc() -> String {
    let output = Command::new("node")
        .arg("-e")
        .arg("console.log(new Date().toISOString())")
        .output();
    match output {
        Ok(result) if result.status.success() => {
            String::from_utf8_lossy(&result.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
#[test]
fn adapter_timing_mode_is_uniform() {
    let selected_mode = TimingMode::EvalPerIteration;
    for engine in EngineKind::all_required() {
        assert_eq!(
            timing_mode_for_engine(engine, selected_mode),
            selected_mode,
            "engine {} diverged from run timing mode",
            engine.as_str()
        );
    }
}

#[cfg(test)]
#[test]
fn adapter_checksum_parity_is_value_based() {
    let mut checksum = 0.0;
    checksum += guard_delta_from_number_or_bool(Some(40.0), Some(false));
    checksum += guard_delta_from_number_or_bool(None, Some(true));
    checksum += guard_delta_from_number_or_bool(None, Some(false));
    checksum += guard_delta_from_number_or_bool(None, None);

    assert_eq!(checksum, 41.0);
    assert_ne!(checksum, 4.0, "checksum must not degrade to loop counters");
}

#[cfg(test)]
#[test]
fn comparator_preflight_metadata_is_complete() {
    let engine_status = vec![
        EngineExecutionMetadata {
            engine: EngineKind::QjsRs.as_str().to_string(),
            status: EngineAvailabilityStatus::Available,
            command: "in-process".to_string(),
            path: None,
            workdir: None,
            version: Some("qjs-rs test".to_string()),
            reason: None,
        },
        EngineExecutionMetadata {
            engine: EngineKind::BoaEngine.as_str().to_string(),
            status: EngineAvailabilityStatus::Available,
            command: "in-process".to_string(),
            path: None,
            workdir: None,
            version: Some("boa test".to_string()),
            reason: None,
        },
        EngineExecutionMetadata {
            engine: EngineKind::NodeJs.as_str().to_string(),
            status: EngineAvailabilityStatus::Available,
            command: "node".to_string(),
            path: Some("/usr/bin/node".to_string()),
            workdir: Some("/tmp".to_string()),
            version: Some("v22.0.0".to_string()),
            reason: None,
        },
        EngineExecutionMetadata {
            engine: EngineKind::QuickJsC.as_str().to_string(),
            status: EngineAvailabilityStatus::Available,
            command: "qjs".to_string(),
            path: Some("/opt/quickjs/qjs".to_string()),
            workdir: Some("/opt/quickjs".to_string()),
            version: Some("QuickJS version 2026-01-01".to_string()),
            reason: None,
        },
    ];

    let reproducibility = ReproducibilityMetadata::for_run_with_engine_status(
        contract::RunProfile::CiLinux,
        Path::new("target/benchmarks/engine-comparison.ci-linux.json"),
        true,
        engine_status,
    );

    assert!(reproducibility.comparator_strict_mode);
    let node = reproducibility
        .engine_status
        .iter()
        .find(|entry| entry.engine == EngineKind::NodeJs.as_str())
        .expect("node metadata should be present");
    assert_eq!(node.status, EngineAvailabilityStatus::Available);
    assert_eq!(node.command, "node");
    assert_eq!(node.path.as_deref(), Some("/usr/bin/node"));
    assert_eq!(node.workdir.as_deref(), Some("/tmp"));
    assert!(node.version.is_some());
}

#[cfg(test)]
#[test]
fn packet_c_output_path_enables_packet_c_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-c.json".to_string(),
            "--allow-missing-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_c_enabled(&cli),
        "packet-c output artifacts must enable packet-c runtime fast path for qjs-rs runs"
    );
}

#[cfg(test)]
#[test]
fn packet_d_output_path_enables_packet_d_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-d.json".to_string(),
            "--allow-missing-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_d_enabled(&cli),
        "packet-d output artifacts must enable packet-d runtime fast path for qjs-rs runs"
    );
    assert!(
        infer_packet_c_enabled(&cli),
        "packet-d output artifacts keep packet-c enabled so packet-d composes with earlier fast paths"
    );
}

#[cfg(test)]
#[test]
fn packet_e_output_path_enables_packet_d_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "ci-linux".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.ci-linux.packet-e.json".to_string(),
            "--strict-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_d_enabled(&cli),
        "packet-e output artifacts must keep packet-d runtime fast path enabled"
    );
    assert!(
        infer_packet_c_enabled(&cli),
        "packet-e output artifacts must keep packet-c runtime fast path enabled"
    );
}

#[cfg(test)]
#[test]
fn packet_f_output_path_enables_packet_d_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-f.json".to_string(),
            "--strict-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_d_enabled(&cli),
        "packet-f output artifacts must keep packet-d runtime fast path enabled"
    );
    assert!(
        infer_packet_c_enabled(&cli),
        "packet-f output artifacts must keep packet-c runtime fast path enabled"
    );
}

#[cfg(test)]
#[test]
fn packet_g_output_path_enables_packet_g_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-g.json".to_string(),
            "--strict-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_g_enabled(&cli),
        "packet-g output artifacts must enable packet-g runtime fast path"
    );
    assert!(
        infer_packet_d_enabled(&cli),
        "packet-g output artifacts must keep packet-d runtime fast path enabled"
    );
    assert!(
        infer_packet_c_enabled(&cli),
        "packet-g output artifacts must keep packet-c runtime fast path enabled"
    );
}

#[cfg(test)]
#[test]
fn packet_h_output_path_enables_packet_h_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-h.smoke.json".to_string(),
            "--strict-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_h_enabled(&cli),
        "packet-h output artifacts must enable packet-h runtime fast path"
    );
    assert!(
        !infer_packet_i_enabled(&cli),
        "packet-h output artifacts must not implicitly enable packet-i revalidation"
    );
    assert!(
        infer_packet_g_enabled(&cli),
        "packet-h output artifacts must keep packet-g runtime fast path enabled"
    );
    assert!(
        infer_packet_d_enabled(&cli),
        "packet-h output artifacts must keep packet-d runtime fast path enabled"
    );
    assert!(
        infer_packet_c_enabled(&cli),
        "packet-h output artifacts must keep packet-c runtime fast path enabled"
    );
}

#[cfg(test)]
#[test]
fn packet_i_output_path_enables_packet_i_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-i.smoke.json".to_string(),
            "--strict-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_i_enabled(&cli),
        "packet-i output artifacts must enable packet-i revalidation runtime behavior"
    );
    assert!(
        infer_packet_h_enabled(&cli),
        "packet-i output artifacts must keep packet-h runtime fast path enabled"
    );
    assert!(
        infer_packet_g_enabled(&cli),
        "packet-i output artifacts must keep packet-g runtime fast path enabled"
    );
    assert!(
        infer_packet_d_enabled(&cli),
        "packet-i output artifacts must keep packet-d runtime fast path enabled"
    );
    assert!(
        infer_packet_c_enabled(&cli),
        "packet-i output artifacts must keep packet-c runtime fast path enabled"
    );
}

#[cfg(test)]
#[test]
fn packet_final_output_path_enables_packet_d_runtime_toggle() {
    let cli = match contract::parse_cli_args_with_env(
        vec![
            "--profile".to_string(),
            "local-dev".to_string(),
            "--output".to_string(),
            "target/benchmarks/engine-comparison.local-dev.packet-final.json".to_string(),
            "--strict-comparators".to_string(),
        ],
        &[],
    )
    .expect("cli args should parse")
    {
        contract::CliParseResult::Run(cli) => *cli,
        contract::CliParseResult::Help => panic!("expected parsed run config"),
    };

    assert!(
        infer_packet_d_enabled(&cli),
        "packet-final output artifacts must keep packet-d runtime fast path enabled"
    );
    assert!(
        infer_packet_c_enabled(&cli),
        "packet-final output artifacts must keep packet-c runtime fast path enabled"
    );
}

fn main() -> Result<()> {
    let cli = match parse_cli_args(env::args().skip(1))? {
        CliParseResult::Help => {
            println!("{}", help_text());
            return Ok(());
        }
        CliParseResult::Run(args) => *args,
    };

    let cases = benchmark_cases();
    let preflight_metadata = preflight_engine_execution(&cli.comparators)?;
    let environment = collect_environment(&preflight_metadata);
    let optimization_descriptor = contract::infer_optimization_descriptor(&cli.output);
    let packet_c_enabled = infer_packet_c_enabled(&cli);
    let packet_d_enabled = infer_packet_d_enabled(&cli);
    let packet_g_enabled = infer_packet_g_enabled(&cli);
    let packet_h_enabled = infer_packet_h_enabled(&cli);
    let packet_i_enabled = infer_packet_i_enabled(&cli);
    let hotspot_attribution_enabled = infer_hotspot_attribution_default(&cli);
    let perf_target = build_perf_target_metadata(&cli.output, &environment);

    println!(
        "Running benchmark suite ({}) with profile={} timing_mode=eval-per-iteration strict_comparators={} hotspot_attribution={} packet_c_enabled={} packet_d_enabled={} packet_g_enabled={} packet_h_enabled={} packet_i_enabled={} optimization_mode={:?} optimization_tag={} packet_id={}: {} cases x {} engines x {} samples ({} iterations/sample)",
        SCHEMA_VERSION,
        cli.run_profile.as_str(),
        cli.comparators.strict_external,
        hotspot_attribution_enabled,
        packet_c_enabled,
        packet_d_enabled,
        packet_g_enabled,
        packet_h_enabled,
        packet_i_enabled,
        optimization_descriptor.mode,
        perf_target.optimization_tag,
        perf_target.packet_id.as_deref().unwrap_or("none"),
        cases.len(),
        EngineKind::all_required().len(),
        cli.config.samples,
        cli.config.iterations
    );
    for entry in &preflight_metadata {
        println!(
            "  comparator {:<10} status={:?} command={} path={} version={}",
            entry.engine,
            entry.status,
            entry.command,
            entry.path.as_deref().unwrap_or("<resolved-at-runtime>"),
            entry.version.as_deref().unwrap_or("<unavailable>")
        );
    }

    let mut case_reports = Vec::with_capacity(cases.len());
    let mut hotspot_per_case: BTreeMap<String, HotspotAttributionCounters> = BTreeMap::new();
    for case in &cases {
        println!("[case:{}] {}", case.definition.id, case.definition.title);
        let wrapped = wrap_script(case.script_body);
        let mut engines = BTreeMap::new();

        for engine in EngineKind::all_required() {
            if !engine_available(engine, &preflight_metadata) {
                println!(
                    "  - {:<11} skipped (comparator unavailable; see reproducibility.engine_status)",
                    engine.as_str()
                );
                continue;
            }
            let warmup = run_engine_case(
                engine,
                RunEngineCaseContext {
                    timing_mode: cli.timing_mode,
                    script: &wrapped,
                    iterations: cli.config.warmup_iterations,
                    comparators: &cli.comparators,
                    hotspot_attribution_enabled,
                    packet_c_enabled,
                    packet_d_enabled,
                    packet_g_enabled,
                    packet_h_enabled,
                    packet_i_enabled,
                },
            )?;

            let mut samples = Vec::with_capacity(cli.config.samples);
            for _ in 0..cli.config.samples {
                let sample = run_engine_case(
                    engine,
                    RunEngineCaseContext {
                        timing_mode: cli.timing_mode,
                        script: &wrapped,
                        iterations: cli.config.iterations,
                        comparators: &cli.comparators,
                        hotspot_attribution_enabled,
                        packet_c_enabled,
                        packet_d_enabled,
                        packet_g_enabled,
                        packet_h_enabled,
                        packet_i_enabled,
                    },
                )?;
                samples.push(sample);
            }
            if engine == EngineKind::QjsRs
                && hotspot_attribution_enabled
                && let Some(case_snapshot) = aggregate_case_hotspot_attribution(&samples)
            {
                hotspot_per_case.insert(case.definition.id.to_string(), case_snapshot);
            }
            let summary = summarize(samples, cli.config.iterations, warmup.guard_checksum);
            println!(
                "  - {:<11} mean={:>8.3}ms median={:>8.3}ms throughput={:>10.2} ops/s",
                engine.as_str(),
                summary.mean_ms,
                summary.median_ms,
                summary.throughput_ops_per_sec
            );
            engines.insert(engine.as_str().to_string(), summary);
        }

        case_reports.push(CaseReport {
            id: case.definition.id.to_string(),
            title: case.definition.title.to_string(),
            description: case.definition.description.to_string(),
            engines,
        });
    }

    let qjs_hotspot_attribution = if hotspot_per_case.is_empty() {
        None
    } else {
        let mut total = HotspotAttributionCounters {
            numeric_ops: 0,
            identifier_resolution: 0,
            identifier_resolution_fallback_scans: 0,
            packet_d_slot_guard_hits: 0,
            packet_d_slot_guard_misses: 0,
            packet_d_slot_guard_revalidate_hits: 0,
            packet_d_slot_guard_revalidate_misses: 0,
            packet_g_name_guard_hits: 0,
            packet_g_name_guard_misses: 0,
            packet_g_name_guard_revalidate_hits: 0,
            packet_g_name_guard_revalidate_misses: 0,
            packet_h_lexical_slot_guard_hits: 0,
            packet_h_lexical_slot_guard_misses: 0,
            array_indexed_property_get: 0,
            array_indexed_property_set: 0,
        };
        for counters in hotspot_per_case.values() {
            merge_hotspot_counters(&mut total, counters);
        }
        Some(HotspotAttributionSnapshot {
            enabled: hotspot_attribution_enabled,
            source: "vm-hotspot-attribution-v1",
            total,
            per_case: hotspot_per_case,
        })
    };

    let report = BenchmarkReport {
        schema_version: SCHEMA_VERSION,
        generated_at_utc: generated_at_utc(),
        run_profile: cli.run_profile,
        timing_mode: cli.timing_mode,
        config: cli.config.clone(),
        reproducibility: ReproducibilityMetadata::for_run_with_engine_status(
            cli.run_profile,
            &cli.output,
            cli.comparators.strict_external,
            preflight_metadata.clone(),
        ),
        environment,
        aggregate: aggregate(&case_reports),
        cases: case_reports,
        perf_target,
        qjs_rs_hotspot_attribution: qjs_hotspot_attribution,
    };

    ensure_output_dir(&cli.output)?;
    fs::write(&cli.output, serde_json::to_vec_pretty(&report)?)?;
    println!("Wrote benchmark results to {}", cli.output.display());
    Ok(())
}
