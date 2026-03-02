#![forbid(unsafe_code)]

use anyhow::{Context as _, Result, anyhow};
use builtins::install_baseline;
use bytecode::{Chunk, compile_script};
use parser::parse_script;
use runtime::{JsValue, Realm};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use vm::{Vm, VmError};

#[derive(Debug, Clone, Copy)]
struct BenchConfig {
    samples: usize,
    iterations: usize,
}

#[derive(Debug, Serialize)]
struct CaseResult {
    case_id: &'static str,
    mean_ms: f64,
    min_ms: f64,
    max_ms: f64,
    checksum: f64,
}

#[derive(Debug, Serialize)]
struct HostInteropReport {
    schema_version: &'static str,
    generated_unix_ms: u128,
    engine: &'static str,
    samples: usize,
    iterations: usize,
    cases: Vec<CaseResult>,
}

fn main() -> Result<()> {
    let config = parse_cli(env::args().skip(1))?;
    let cases = vec![
        bench_js_to_rust_calls(config).context("case js-to-rust-call")?,
        bench_rust_to_js_callbacks(config).context("case rust-to-js-call")?,
        bench_rust_constructor_gc(config).context("case rust-constructor-gc")?,
        bench_async_promise_jobs(config).context("case async-promise-jobs")?,
    ];
    let report = HostInteropReport {
        schema_version: "host-interop.v1",
        generated_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0),
        engine: "qjs-rs",
        samples: config.samples,
        iterations: config.iterations,
        cases,
    };
    let output = output_path();
    ensure_output_dir(&output)?;
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Wrote host interop benchmark report to {}",
        output.display()
    );
    Ok(())
}

fn parse_cli<I>(args: I) -> Result<BenchConfig>
where
    I: IntoIterator<Item = String>,
{
    let mut samples = 7usize;
    let mut iterations = 100usize;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--samples" => {
                samples = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value after --samples"))?
                    .parse::<usize>()
                    .context("invalid --samples value")?;
            }
            "--iterations" => {
                iterations = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value after --iterations"))?
                    .parse::<usize>()
                    .context("invalid --iterations value")?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: cargo run -p benchmarks --bin host_interop -- [--samples N] [--iterations N]"
                );
                std::process::exit(0);
            }
            unknown => return Err(anyhow!("unknown argument: {unknown}")),
        }
    }
    if samples == 0 || iterations == 0 {
        return Err(anyhow!("samples and iterations must be > 0"));
    }
    Ok(BenchConfig {
        samples,
        iterations,
    })
}

fn bench_js_to_rust_calls(config: BenchConfig) -> Result<CaseResult> {
    let script = compile_script_from_source(
        "let acc = 0; for (let i = 0; i < 500; i = i + 1) { acc = rustAdd(acc, 1); } acc;",
    )?;
    let mut sample_ms = Vec::with_capacity(config.samples);
    let mut checksum = 0.0;

    for _ in 0..config.samples {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        let mut vm = Vm::default();
        vm_into_anyhow(vm.define_global_host_callback(
            &realm,
            "rustAdd",
            2.0,
            false,
            |_vm, _this_arg, args, _realm, _strict| {
                let lhs = args.first().map_or(0.0, extract_number);
                let rhs = args.get(1).map_or(0.0, extract_number);
                Ok(JsValue::Number(lhs + rhs))
            },
        ))?;

        let start = Instant::now();
        let mut local_checksum = 0.0;
        for _ in 0..config.iterations {
            let value = vm_into_anyhow(vm.execute_in_realm_persistent(&script, &realm))?;
            local_checksum += extract_number(&value);
        }
        sample_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        checksum += local_checksum;
    }

    Ok(summarize_case("js-to-rust-call", sample_ms, checksum))
}

fn bench_rust_to_js_callbacks(config: BenchConfig) -> Result<CaseResult> {
    let script = compile_script_from_source(
        "function addOne(v) { return v + 1; }\
         let acc = 0;\
         for (let i = 0; i < 500; i = i + 1) {\
           acc = rustCallJs(addOne, acc);\
         }\
         acc;",
    )?;
    let mut sample_ms = Vec::with_capacity(config.samples);
    let mut checksum = 0.0;

    for _ in 0..config.samples {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        let mut vm = Vm::default();
        vm_into_anyhow(vm.define_global_host_callback(
            &realm,
            "rustCallJs",
            2.0,
            false,
            |vm, _this_arg, args, realm, strict| {
                let callback = args.first().cloned().unwrap_or(JsValue::Undefined);
                let value = args.get(1).cloned().unwrap_or(JsValue::Undefined);
                vm.call_function_value(
                    callback,
                    Some(JsValue::Undefined),
                    vec![value],
                    realm,
                    strict,
                )
            },
        ))?;

        let start = Instant::now();
        let mut local_checksum = 0.0;
        for _ in 0..config.iterations {
            let value = vm_into_anyhow(vm.execute_in_realm_persistent(&script, &realm))?;
            local_checksum += extract_number(&value);
        }
        sample_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        checksum += local_checksum;
    }

    Ok(summarize_case("rust-to-js-call", sample_ms, checksum))
}

fn bench_rust_constructor_gc(config: BenchConfig) -> Result<CaseResult> {
    struct RustBigObj {
        payload: Vec<u8>,
        drop_counter: Arc<AtomicUsize>,
    }

    impl Drop for RustBigObj {
        fn drop(&mut self) {
            let _ = self.payload.len();
            self.drop_counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    let script = compile_script_from_source(
        "for (let i = 0; i < 128; i = i + 1) { { const a = new RustBigObj(); } } 0;",
    )?;
    let mut sample_ms = Vec::with_capacity(config.samples);
    let mut checksum = 0.0;

    for _ in 0..config.samples {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        let mut vm = Vm::default();
        let drop_counter = Arc::new(AtomicUsize::new(0));
        let drop_counter_for_callback = Arc::clone(&drop_counter);
        vm_into_anyhow(vm.define_global_host_callback(
            &realm,
            "RustBigObj",
            0.0,
            true,
            move |vm, this_arg, _args, _realm, _strict| {
                let this_obj =
                    this_arg.ok_or(VmError::TypeError("HostCallback:MissingConstructorThis"))?;
                vm.bind_opaque_data(
                    &this_obj,
                    RustBigObj {
                        payload: vec![0u8; 1024 * 1024],
                        drop_counter: Arc::clone(&drop_counter_for_callback),
                    },
                )?;
                Ok(this_obj)
            },
        ))?;

        let start = Instant::now();
        for _ in 0..config.iterations {
            let _ = vm_into_anyhow(vm.execute_in_realm_persistent(&script, &realm))?;
            let _ = vm.collect_garbage(&realm);
        }
        sample_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        checksum += drop_counter.load(Ordering::SeqCst) as f64;
    }

    Ok(summarize_case("rust-constructor-gc", sample_ms, checksum))
}

fn bench_async_promise_jobs(config: BenchConfig) -> Result<CaseResult> {
    let script = compile_script_from_source(
        "async function __bench_base(v) { return v; }\
         globalThis.__bench_sum = 0;\
         for (let i = 0; i < 200; i = i + 1) {\
           __bench_base(i).then(function(v) {\
             globalThis.__bench_sum = globalThis.__bench_sum + v;\
           });\
         }\
         0;",
    )?;
    let readback = compile_script_from_source("globalThis.__bench_sum;")?;
    let mut sample_ms = Vec::with_capacity(config.samples);
    let mut checksum = 0.0;

    for _ in 0..config.samples {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        let mut vm = Vm::default();
        let start = Instant::now();
        let mut local_checksum = 0.0;
        for _ in 0..config.iterations {
            let _ = vm_into_anyhow(vm.execute_in_realm_persistent(&script, &realm))?;
            while vm.has_pending_promise_jobs() {
                let _ = vm_into_anyhow(vm.drain_promise_jobs(usize::MAX, &realm, false))?;
            }
            let value = vm_into_anyhow(vm.execute_in_realm_persistent(&readback, &realm))?;
            local_checksum += extract_number(&value);
        }
        sample_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        checksum += local_checksum;
    }

    Ok(summarize_case("async-promise-jobs", sample_ms, checksum))
}

fn compile_script_from_source(source: &str) -> Result<Chunk> {
    let parsed = parse_script(source).map_err(|err| anyhow!("parse error: {}", err.message))?;
    Ok(compile_script(&parsed))
}

fn summarize_case(case_id: &'static str, sample_ms: Vec<f64>, checksum: f64) -> CaseResult {
    let mean_ms = sample_ms.iter().sum::<f64>() / sample_ms.len() as f64;
    let min_ms = sample_ms.iter().copied().fold(f64::INFINITY, f64::min);
    let max_ms = sample_ms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    CaseResult {
        case_id,
        mean_ms,
        min_ms,
        max_ms,
        checksum,
    }
}

fn extract_number(value: &JsValue) -> f64 {
    match value {
        JsValue::Number(value) => *value,
        JsValue::Bool(true) => 1.0,
        JsValue::Bool(false) => 0.0,
        _ => 0.0,
    }
}

fn output_path() -> PathBuf {
    PathBuf::from("target/benchmarks/host-interop.local-dev.json")
}

fn ensure_output_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn vm_into_anyhow<T>(result: std::result::Result<T, VmError>) -> Result<T> {
    result.map_err(|error| anyhow!("vm error: {error:?}"))
}
