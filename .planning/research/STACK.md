# Stack Research

**Domain:** qjs-rs v1.1 Performance Acceleration (benchmark baseline + profiling + optimization + CI regression gating)
**Researched:** 2026-02-27
**Confidence:** HIGH (tooling choices), MEDIUM (optimization-library adoption depends on measured hotspots)

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Existing `crates/benchmarks` cross-engine harness (upgrade dependencies) | `boa_engine 0.21.0` (from `0.20`), JSON report schema v1 | Keep qjs-rs/boa/node/quickjs-c macro-bench results reproducible and comparable | Lowest integration risk because the harness already exists; only extend it with stronger metadata (toolchain/CPU/run mode) and threshold checks instead of replacing it. |
| `criterion` | `0.8.2` | Statistical Rust microbenchmarks for VM/runtime hot paths (opcode dispatch, property lookup, call frames, arrays) | Standard Rust choice for stable confidence intervals and noise handling; integrates as `benches/` without touching runtime semantics. |
| `iai-callgrind` | `0.16.1` | Deterministic instruction-level benchmarking (instructions, cache refs/misses) for regression proof | Complements wall-clock benchmarks in noisy CI by tracking instruction deltas; ideal for gating “same workload, more instructions” regressions. |
| Engine baseline pinning | Node.js `v24.14.0` LTS, QuickJS `2025-09-13-2`, Rust `1.93.1` | Freeze external comparators for milestone-wide apples-to-apples runs | Prevents accidental benchmark drift from external engine upgrades; makes PR vs baseline comparisons auditable. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `pprof` | `0.15.0` | CPU profiling and flamegraph export from Rust benchmark binaries | Use during hotspot diagnosis after a benchmark regression is detected; keep behind a `perf-prof` feature/dev-only dependency path. |
| `tracing` + `tracing-subscriber` | `0.1.44` + `0.3.22` | Structured span/timing instrumentation for VM execution phases | Use for attribution (parse vs compile vs execute vs GC) in perf investigations; disable by default in release/perf-gate runs to avoid observer effect. |
| `smallvec` | `1.15.1` | Inline small-array optimization for short-lived vectors (args, stack scratch, property iteration buffers) | Introduce only where profiles show allocation churn in tiny vectors; avoid broad replacement to keep code clarity. |
| `rustc-hash` | `2.1.1` | Faster non-cryptographic hashing for internal engine maps | Use in hot internal maps (symbols/shapes/caches) where DOS-resistant hashing is not required by host-facing APIs. |
| `bumpalo` | `3.20.2` | Arena allocation for compile-time temporaries (parser/bytecode lowering) | Use for short-lived compiler allocations; do **not** use for GC-managed runtime objects. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `flamegraph` (`cargo flamegraph`) | Visualize CPU hotspots from benchmark scenarios | Pin `0.6.11`; requires Linux perf tooling. Keep as developer diagnostics, not CI gate output. |
| `cargo-bloat` | Identify code-size growth that harms i-cache and cold-start | Pin `0.12.1`; run on optimized builds before/after optimization PRs. |
| CI perf-compare script (repo-local Python) | Enforce explicit regression thresholds against checked-in baseline JSON | Keep logic in-repo for reviewability; gate on relative thresholds (per-case and aggregate), not absolute ms only. |
| GitHub Actions artifacts + summary | Persist benchmark/profiling outputs and show PR deltas | Upload raw JSON + rendered markdown diff so regressions are inspectable without rerunning locally. |

## Installation

```bash
# Benchmark + profiling crates (workspace-scoped where appropriate)
cargo add -p benchmarks criterion@0.8.2 --dev
cargo add -p benchmarks iai-callgrind@0.16.1 --dev
cargo add -p benchmarks pprof@0.15.0 --features flamegraph
cargo add -p vm tracing@0.1.44
cargo add -p vm tracing-subscriber@0.3.22

# Optional hotspot-driven optimization crates (add only after profiling evidence)
cargo add -p vm smallvec@1.15.1
cargo add -p vm rustc-hash@2.1.1
cargo add -p bytecode bumpalo@3.20.2

# Dev tools
cargo install flamegraph --version 0.6.11
cargo install cargo-bloat --version 0.12.1

# Linux CI/local profiler prerequisites
sudo apt-get update && sudo apt-get install -y valgrind linux-perf
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `criterion 0.8.2` for wall-clock microbench | `divan 0.1.21` | Use `divan` only if benchmark compile time becomes a bottleneck and statistical depth can be reduced. |
| `iai-callgrind 0.16.1` for deterministic instruction metrics | plain `perf stat` scripts | Use `perf stat` for ad-hoc local triage when Valgrind is unavailable, but keep CI gates on reproducible harness output. |
| Repo-local perf gate script + baseline JSON | external SaaS benchmarking dashboards | Use SaaS only if long-term trend dashboards are required; keep milestone acceptance independent of third-party services. |
| `pprof` + `flamegraph` | only manual logging/timers | Manual timers are acceptable for quick checks, but not sufficient for root-cause analysis in VM hotspots. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Runtime-core C/C++ profiler/allocator FFI dependencies | Violates pure-Rust runtime boundary and increases maintenance/portability risk | Rust-native profiling + allocation optimizations (`pprof`, `smallvec`, `bumpalo`, targeted refactors). |
| JIT introduction in v1.1 (Cranelift/LLVM JIT path) | Massive scope jump; obscures baseline improvements needed for interpreter hot paths | Keep v1.1 focused on interpreter/runtime/bytecode optimizations with measurable benchmark wins. |
| Always-on tracing/profiling in benchmark gate runs | Distorts timing and makes regression thresholds noisy | Compile instrumentation behind feature flags and run CI perf gates with instrumentation off. |
| Global allocator swaps as first-line optimization (`jemalloc`/`mimalloc`) | Can hide algorithmic regressions and create platform-specific variance | First optimize data structures and opcode/runtime algorithms; evaluate allocator changes only as explicit follow-up experiments. |

## Stack Patterns by Variant

**If running PR fast-gate (every pull request):**
- Run reduced-sample macro benchmarks + semantic non-regression tests.
- Compare against checked-in baseline with conservative thresholds (e.g., fail if aggregate regresses >5% or any key case >8%).
- Because fast feedback is needed while still blocking obvious regressions.

**If running nightly/deep performance lane:**
- Run full macro suite (`qjs-rs`, `boa`, `node`, `quickjs-c`) + `criterion` microbenches + `iai-callgrind`.
- Publish artifacts (raw JSON, callgrind outputs, flamegraph SVG) and trend summary.
- Because deep runs provide high-confidence optimization guidance without slowing normal PR flow.

**If investigating a hotspot locally:**
- Reproduce with benchmark case id + fixed iterations/samples, then run `pprof`/`flamegraph` and optional `tracing` spans.
- Apply one optimization at a time and rerun both semantic and perf suites.
- Because single-variable changes preserve causal confidence.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `criterion@0.8.2` | Rust stable `1.93.1` (workspace `rust-version >=1.85`) | Stable benchmark harness integration via `cargo bench`/custom benches. |
| `iai-callgrind@0.16.1` | Linux CI with Valgrind installed | Best in dedicated Linux perf jobs; avoid Windows/macOS-only perf gating. |
| `pprof@0.15.0` | `flamegraph@0.6.11` tooling | Use for profiling artifacts, not mandatory in default CI path. |
| `boa_engine@0.21.0` | qjs-rs benchmark comparator harness | Update from `0.20` to keep v1.1 baseline current and explicit. |
| Node.js `v24.14.0` + QuickJS `2025-09-13-2` | Cross-engine baseline suite | Pin versions in benchmark metadata to prevent silent comparator drift. |

## Sources

- `.planning/PROJECT.md` — v1.1 milestone scope and constraints (pure Rust core, performance goals).
- `Cargo.toml`, `crates/benchmarks/Cargo.toml`, `crates/benchmarks/src/main.rs`, `.github/workflows/ci.yml` — current local stack and integration baseline.
- https://crates.io/crates/criterion — verified current crate version (`0.8.2`).
- https://crates.io/crates/iai-callgrind — verified current crate version (`0.16.1`).
- https://crates.io/crates/pprof — verified current crate version (`0.15.0`).
- https://crates.io/crates/flamegraph — verified current tool version (`0.6.11`).
- https://crates.io/crates/cargo-bloat — verified current tool version (`0.12.1`).
- https://crates.io/crates/boa_engine — verified current comparator version (`0.21.0`).
- https://crates.io/crates/tracing and https://crates.io/crates/tracing-subscriber — instrumentation versions (`0.1.44`, `0.3.22`).
- https://crates.io/crates/smallvec, https://crates.io/crates/rustc-hash, https://crates.io/crates/bumpalo — optimization-library versions.
- https://nodejs.org/dist/index.json — current Node.js LTS line/version (`v24.14.0`, retrieved 2026-02-27).
- https://bellard.org/quickjs/ — latest QuickJS source release entry (`quickjs-2025-09-13-2.tar.xz`).

---
*Stack research for: qjs-rs v1.1 performance acceleration*
*Researched: 2026-02-27*
