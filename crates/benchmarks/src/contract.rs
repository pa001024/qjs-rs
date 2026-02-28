use anyhow::{anyhow, bail, Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const SCHEMA_VERSION: &str = "bench.v1";
pub const GUARD_CHECKSUM_MODE: &str = "value-checksum-v1";
pub const PERF_TARGET_POLICY_ID: &str = "phase11-perf03-local-dev-eval-per-iteration";
pub const PERF_TARGET_AUTHORITY_PROFILE: RunProfile = RunProfile::LocalDev;
pub const PERF_TARGET_AUTHORITY_TIMING_MODE: TimingMode = TimingMode::EvalPerIteration;
pub const ENV_NODE_COMMAND: &str = "BENCH_NODE_COMMAND";
pub const ENV_NODE_PATH: &str = "BENCH_NODE_PATH";
pub const ENV_NODE_WORKDIR: &str = "BENCH_NODE_WORKDIR";
pub const ENV_QUICKJS_COMMAND: &str = "BENCH_QUICKJS_COMMAND";
pub const ENV_QUICKJS_PATH: &str = "BENCH_QUICKJS_PATH";
pub const ENV_QUICKJS_WORKDIR: &str = "BENCH_QUICKJS_WORKDIR";
pub const ENV_STRICT_COMPARATORS: &str = "BENCH_STRICT_COMPARATORS";
pub const REQUIRED_CLOSURE_COMPARATORS: [&str; 2] = ["qjs-rs", "boa-engine"];
pub const OPTIONAL_CLOSURE_COMPARATORS: [&str; 2] = ["quickjs-c", "nodejs"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum EngineKind {
    QjsRs,
    BoaEngine,
    NodeJs,
    QuickJsC,
}

impl EngineKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EngineKind::QjsRs => "qjs-rs",
            EngineKind::BoaEngine => "boa-engine",
            EngineKind::NodeJs => "nodejs",
            EngineKind::QuickJsC => "quickjs-c",
        }
    }

    pub fn all_required() -> [EngineKind; 4] {
        [
            EngineKind::QjsRs,
            EngineKind::BoaEngine,
            EngineKind::NodeJs,
            EngineKind::QuickJsC,
        ]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunProfile {
    LocalDev,
    CiLinux,
}

impl RunProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            RunProfile::LocalDev => "local-dev",
            RunProfile::CiLinux => "ci-linux",
        }
    }

    pub fn default_config(self) -> BenchmarkConfig {
        match self {
            RunProfile::LocalDev => BenchmarkConfig {
                iterations: 200,
                samples: 7,
                warmup_iterations: 3,
            },
            RunProfile::CiLinux => BenchmarkConfig {
                iterations: 400,
                samples: 9,
                warmup_iterations: 5,
            },
        }
    }

    pub fn default_output_path(self) -> PathBuf {
        PathBuf::from(format!(
            "target/benchmarks/engine-comparison.{}.json",
            self.as_str()
        ))
    }

    pub fn strict_comparators_default(self) -> bool {
        match self {
            RunProfile::LocalDev => false,
            RunProfile::CiLinux => true,
        }
    }
}

impl FromStr for RunProfile {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "local-dev" => Ok(Self::LocalDev),
            "ci-linux" => Ok(Self::CiLinux),
            unknown => bail!("unknown run profile: {unknown} (expected local-dev or ci-linux)"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TimingMode {
    EvalPerIteration,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OptimizationMode {
    Baseline,
    Packet,
    Unspecified,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerfTargetMetadata {
    pub policy_id: &'static str,
    pub authoritative_run_profile: RunProfile,
    pub authoritative_timing_mode: TimingMode,
    pub same_host_required: bool,
    pub host_fingerprint: String,
    pub optimization_mode: OptimizationMode,
    pub optimization_tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_id: Option<String>,
    pub required_comparators: Vec<&'static str>,
    pub optional_comparators: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HotspotAttributionCounters {
    pub numeric_ops: u64,
    pub identifier_resolution: u64,
    pub array_indexed_property_get: u64,
    pub array_indexed_property_set: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HotspotAttributionSnapshot {
    pub enabled: bool,
    pub source: &'static str,
    pub total: HotspotAttributionCounters,
    pub per_case: BTreeMap<String, HotspotAttributionCounters>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationDescriptor {
    pub mode: OptimizationMode,
    pub tag: String,
    pub packet_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct RequiredCaseDefinition {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

pub const REQUIRED_CASES: [RequiredCaseDefinition; 4] = [
    RequiredCaseDefinition {
        id: "arith-loop",
        title: "Arithmetic Loop",
        description: "integer arithmetic in a hot for-loop",
    },
    RequiredCaseDefinition {
        id: "fib-iterative",
        title: "Iterative Fibonacci",
        description: "function call + loop + assignments",
    },
    RequiredCaseDefinition {
        id: "array-sum",
        title: "Array Build and Sum",
        description: "array writes + reads + accumulation",
    },
    RequiredCaseDefinition {
        id: "json-roundtrip",
        title: "JSON Roundtrip",
        description: "JSON.stringify + JSON.parse baseline path",
    },
];

pub fn required_case_catalog() -> &'static [RequiredCaseDefinition] {
    &REQUIRED_CASES
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkConfig {
    pub iterations: usize,
    pub samples: usize,
    pub warmup_iterations: usize,
}

impl BenchmarkConfig {
    pub fn validate(&self) -> Result<()> {
        if self.iterations == 0 || self.samples == 0 || self.warmup_iterations == 0 {
            bail!("iterations, samples, and warmup_iterations must be > 0");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ComparatorTarget {
    pub command: String,
    pub path: Option<PathBuf>,
    pub workdir: Option<PathBuf>,
}

impl ComparatorTarget {
    pub fn executable(&self) -> String {
        self.path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| self.command.clone())
    }
}

#[derive(Debug, Clone)]
pub struct ComparatorConfig {
    pub node: ComparatorTarget,
    pub quickjs: ComparatorTarget,
    pub strict_external: bool,
}

#[derive(Debug)]
pub struct CliArgs {
    pub run_profile: RunProfile,
    pub output: PathBuf,
    pub config: BenchmarkConfig,
    pub timing_mode: TimingMode,
    pub comparators: ComparatorConfig,
    pub hotspot_attribution_override: Option<bool>,
}

#[derive(Debug)]
pub enum CliParseResult {
    Run(CliArgs),
    Help,
}

pub fn parse_cli_args<I>(args: I) -> Result<CliParseResult>
where
    I: IntoIterator<Item = String>,
{
    parse_cli_args_with_lookup(args, read_non_empty_env)
}

fn parse_cli_args_with_lookup<I, F>(args: I, env_lookup: F) -> Result<CliParseResult>
where
    I: IntoIterator<Item = String>,
    F: Fn(&str) -> Option<String>,
{
    let mut run_profile = RunProfile::LocalDev;
    let mut iterations_override: Option<usize> = None;
    let mut samples_override: Option<usize> = None;
    let mut warmup_override: Option<usize> = None;
    let mut output: Option<PathBuf> = None;
    let mut node_command: Option<String> = None;
    let mut node_path: Option<PathBuf> = None;
    let mut node_workdir: Option<PathBuf> = None;
    let mut quickjs_command: Option<String> = None;
    let mut quickjs_path: Option<PathBuf> = None;
    let mut quickjs_workdir: Option<PathBuf> = None;
    let mut strict_override: Option<bool> = None;
    let mut hotspot_attribution_override: Option<bool> = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value after --profile"))?;
                run_profile = RunProfile::from_str(&value)?;
            }
            "--iterations" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value after --iterations"))?;
                iterations_override = Some(
                    value
                        .parse::<usize>()
                        .context("invalid --iterations value")?,
                );
            }
            "--samples" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value after --samples"))?;
                samples_override = Some(value.parse::<usize>().context("invalid --samples value")?);
            }
            "--warmup-iterations" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value after --warmup-iterations"))?;
                warmup_override = Some(
                    value
                        .parse::<usize>()
                        .context("invalid --warmup-iterations value")?,
                );
            }
            "--output" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value after --output"))?;
                output = Some(PathBuf::from(value));
            }
            "--node-command" => {
                node_command = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value after --node-command"))?,
                );
            }
            "--node-path" => {
                node_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value after --node-path"))?,
                ));
            }
            "--node-workdir" => {
                node_workdir =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow!("missing value after --node-workdir")
                    })?));
            }
            "--quickjs-command" => {
                quickjs_command = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value after --quickjs-command"))?,
                );
            }
            "--quickjs-path" => {
                quickjs_path =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow!("missing value after --quickjs-path")
                    })?));
            }
            "--quickjs-workdir" => {
                quickjs_workdir =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow!("missing value after --quickjs-workdir")
                    })?));
            }
            "--strict-comparators" => {
                strict_override = Some(true);
            }
            "--allow-missing-comparators" => {
                strict_override = Some(false);
            }
            "--hotspot-attribution" => {
                hotspot_attribution_override = Some(true);
            }
            "--no-hotspot-attribution" => {
                hotspot_attribution_override = Some(false);
            }
            "--help" | "-h" => return Ok(CliParseResult::Help),
            unknown => bail!("unknown argument: {unknown}"),
        }
    }

    let mut config = run_profile.default_config();
    if let Some(iterations) = iterations_override {
        config.iterations = iterations;
    }
    if let Some(samples) = samples_override {
        config.samples = samples;
    }
    if let Some(warmup_iterations) = warmup_override {
        config.warmup_iterations = warmup_iterations;
    }
    config.validate()?;

    let node = resolve_comparator_target(
        node_command,
        node_path,
        node_workdir,
        ENV_NODE_COMMAND,
        ENV_NODE_PATH,
        ENV_NODE_WORKDIR,
        "node",
        &env_lookup,
    );
    let quickjs = resolve_comparator_target(
        quickjs_command,
        quickjs_path,
        quickjs_workdir,
        ENV_QUICKJS_COMMAND,
        ENV_QUICKJS_PATH,
        ENV_QUICKJS_WORKDIR,
        "qjs",
        &env_lookup,
    );
    let strict_external = strict_override
        .or_else(|| read_env_bool(ENV_STRICT_COMPARATORS, &env_lookup))
        .unwrap_or_else(|| run_profile.strict_comparators_default());

    Ok(CliParseResult::Run(CliArgs {
        run_profile,
        output: output.unwrap_or_else(|| run_profile.default_output_path()),
        config,
        timing_mode: TimingMode::EvalPerIteration,
        comparators: ComparatorConfig {
            node,
            quickjs,
            strict_external,
        },
        hotspot_attribution_override,
    }))
}

#[cfg(test)]
pub fn parse_cli_args_with_env<I>(args: I, env_pairs: &[(&str, &str)]) -> Result<CliParseResult>
where
    I: IntoIterator<Item = String>,
{
    let env_map = env_pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect::<BTreeMap<_, _>>();
    parse_cli_args_with_lookup(args, move |key| env_map.get(key).cloned())
}

pub fn help_text() -> &'static str {
    "Usage: cargo run -p benchmarks -- [--profile local-dev|ci-linux] [--iterations N] [--samples N] [--warmup-iterations N] [--output FILE] [--node-command CMD] [--node-path PATH] [--node-workdir DIR] [--quickjs-command CMD] [--quickjs-path PATH] [--quickjs-workdir DIR] [--strict-comparators|--allow-missing-comparators] [--hotspot-attribution|--no-hotspot-attribution]"
}

#[derive(Debug, Serialize)]
pub struct EnvironmentInfo {
    pub os: String,
    pub arch: String,
    pub cpu_parallelism: usize,
    pub rustc: String,
    pub node: String,
    pub quickjs_c: String,
}

#[derive(Debug, Serialize)]
pub struct CaseEngineResult {
    pub sample_ms: Vec<f64>,
    pub mean_ms: f64,
    pub median_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub stddev_ms: f64,
    pub throughput_ops_per_sec: f64,
    pub guard_checksum_mode: &'static str,
    pub warmup_guard_checksum: f64,
    pub sample_guard_checksums: Vec<f64>,
    pub mean_guard_checksum: f64,
    pub guard_checksum_consistent: bool,
}

#[derive(Debug, Serialize)]
pub struct CaseReport {
    pub id: String,
    pub title: String,
    pub description: String,
    pub engines: BTreeMap<String, CaseEngineResult>,
}

#[derive(Debug, Serialize)]
pub struct AggregateReport {
    pub mean_ms_per_engine: BTreeMap<String, f64>,
    pub relative_to_qjs_rs: BTreeMap<String, f64>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EngineAvailabilityStatus {
    Available,
    Missing,
    Unsupported,
}

#[derive(Debug, Serialize, Clone)]
pub struct EngineExecutionMetadata {
    pub engine: String,
    pub status: EngineAvailabilityStatus,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OutputPolicy {
    pub default_path: String,
    pub effective_path: String,
}

#[derive(Debug, Serialize)]
pub struct ReproducibilityMetadata {
    pub required_engines: Vec<String>,
    pub required_case_ids: Vec<String>,
    pub output_policy: OutputPolicy,
    pub comparator_strict_mode: bool,
    pub engine_status: Vec<EngineExecutionMetadata>,
}

impl ReproducibilityMetadata {
    #[allow(dead_code)]
    pub fn for_run(run_profile: RunProfile, effective_output: &Path) -> Self {
        let default_status = EngineKind::all_required()
            .into_iter()
            .map(|engine| EngineExecutionMetadata {
                engine: engine.as_str().to_string(),
                status: EngineAvailabilityStatus::Available,
                command: "unknown".to_string(),
                path: None,
                workdir: None,
                version: None,
                reason: None,
            })
            .collect();
        Self::for_run_with_engine_status(run_profile, effective_output, false, default_status)
    }

    pub fn for_run_with_engine_status(
        run_profile: RunProfile,
        effective_output: &Path,
        comparator_strict_mode: bool,
        engine_status: Vec<EngineExecutionMetadata>,
    ) -> Self {
        Self {
            required_engines: EngineKind::all_required()
                .into_iter()
                .map(|engine| engine.as_str().to_string())
                .collect(),
            required_case_ids: required_case_catalog()
                .iter()
                .map(|case| case.id.to_string())
                .collect(),
            output_policy: OutputPolicy {
                default_path: run_profile.default_output_path().display().to_string(),
                effective_path: effective_output.display().to_string(),
            },
            comparator_strict_mode,
            engine_status,
        }
    }
}

fn read_non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_env_bool<F>(key: &str, env_lookup: &F) -> Option<bool>
where
    F: Fn(&str) -> Option<String>,
{
    env_lookup(key).map(|value| {
        let normalized = value.to_ascii_lowercase();
        matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
    })
}

fn resolve_comparator_target<F>(
    cli_command: Option<String>,
    cli_path: Option<PathBuf>,
    cli_workdir: Option<PathBuf>,
    env_command_key: &str,
    env_path_key: &str,
    env_workdir_key: &str,
    default_command: &str,
    env_lookup: &F,
) -> ComparatorTarget
where
    F: Fn(&str) -> Option<String>,
{
    let path = cli_path.or_else(|| env_lookup(env_path_key).map(PathBuf::from));
    let command = cli_command
        .or_else(|| env_lookup(env_command_key))
        .unwrap_or_else(|| default_command.to_string());
    let workdir = cli_workdir.or_else(|| env_lookup(env_workdir_key).map(PathBuf::from));
    ComparatorTarget {
        command,
        path,
        workdir,
    }
}

#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub schema_version: &'static str,
    pub generated_at_utc: String,
    pub run_profile: RunProfile,
    pub timing_mode: TimingMode,
    pub config: BenchmarkConfig,
    pub reproducibility: ReproducibilityMetadata,
    pub environment: EnvironmentInfo,
    pub cases: Vec<CaseReport>,
    pub aggregate: AggregateReport,
    pub perf_target: PerfTargetMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qjs_rs_hotspot_attribution: Option<HotspotAttributionSnapshot>,
}

pub fn infer_optimization_descriptor(output_path: &Path) -> OptimizationDescriptor {
    let filename = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if let Some(index) = filename.find("packet-") {
        let raw_packet = &filename[index..];
        let packet_id = raw_packet.trim_end_matches(".json").to_string();
        return OptimizationDescriptor {
            mode: OptimizationMode::Packet,
            tag: packet_id.clone(),
            packet_id: Some(packet_id),
        };
    }

    if filename.contains("phase11-baseline") {
        return OptimizationDescriptor {
            mode: OptimizationMode::Baseline,
            tag: "phase11-baseline".to_string(),
            packet_id: None,
        };
    }

    OptimizationDescriptor {
        mode: OptimizationMode::Unspecified,
        tag: "unlabeled".to_string(),
        packet_id: None,
    }
}
