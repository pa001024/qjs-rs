# Project Research Summary

**Project:** qjs-rs
**Domain:** Pure-Rust JavaScript runtime (v1.1 performance acceleration milestone)
**Researched:** 2026-02-27
**Confidence:** HIGH (with execution-time validation gaps noted)

## Executive Summary

qjs-rs v1.1 is a performance-acceleration milestone on top of a completed v1.0 semantic baseline. The research strongly supports an evidence-first optimization strategy: stabilize a reproducible cross-engine benchmark contract, instrument hotspot attribution, then apply guarded optimizations in `bytecode -> vm -> runtime` without changing observable JavaScript behavior.

The recommended approach is to treat performance as a governed pipeline, not isolated code tuning. That means pinned comparator versions (`boa-engine`, `quickjs-c`, `nodejs`), versioned benchmark schemas/artifacts, optimization-on/off semantic parity checks, and CI policy gates that enforce both performance and correctness. This is the shortest path to the milestone goal (`qjs-rs <= boa-engine` aggregate latency) while preserving QuickJS-aligned semantics.

The main risks are benchmark invalidity (apples-to-oranges runs), noise-driven decisions, and semantic drift from aggressive fast paths. Mitigation is explicit: benchmark fairness contract, variance-aware threshold policy, fast-path guard+fallback design, and mandatory dual-gate evidence (perf + semantic) for every optimization PR.

## Key Findings

### Recommended Stack

Use the existing `crates/benchmarks` harness as the control plane and extend it rather than replacing it. Pair statistical benchmarking (`criterion`) with deterministic instruction-level checks (`iai-callgrind`) and keep profiling/instrumentation (`pprof`, `tracing`) feature-gated for investigation rather than always-on CI.

Optimization libraries (`smallvec`, `rustc-hash`, `bumpalo`) should be introduced only with measured hotspot evidence, not preemptively. Comparator/toolchain versions must be pinned and stored in artifacts to prevent baseline drift.

**Core technologies:**
- Existing `crates/benchmarks` harness + schema v1: reproducible cross-engine macrobench runner — lowest integration risk and already aligned with repo workflow.
- `criterion 0.8.2`: statistical microbenchmarks for VM/runtime hot paths — robust variance handling and standard Rust ecosystem choice.
- `iai-callgrind 0.16.1`: deterministic instruction/regression signal — reduces CI noise and catches non-time regressions.

### Expected Features

v1.1 should be framed as performance governance + targeted optimization, not architecture rewrite.

**Must have (table stakes):**
- Reproducible cross-engine baseline harness with machine-readable artifacts — required for credible milestone claims.
- Representative hot-path suite (arith/array/call/JSON families) with stable run controls — required for meaningful trend and threshold gates.
- Targeted VM/runtime/bytecode optimizations with before/after hotspot evidence and semantic non-regression proof.
- CI perf regression gates with explicit thresholds while preserving existing semantic/governance gates.

**Should have (competitive):**
- Dual-gate PR contract (perf delta + semantic delta) as a merge requirement.
- Layered optimization playbook (`bytecode -> vm -> runtime`) to improve iteration speed and rollback safety.
- Stable in-repo evidence pack (JSON + markdown + chart) for auditability and long-term trend tracking.

**Defer (v2+):**
- Value representation overhaul (e.g., NaN-boxing migration).
- Tiered execution/JIT experiments.
- GC strategy redesign beyond telemetry-backed incremental tuning.

### Architecture Approach

Adopt a control-plane architecture around the existing engine execution pipeline: benchmark/instrument/report/gate layers wrap (not replace) `parser -> bytecode -> vm -> runtime -> builtins`. New work should be localized to `bytecode` opt passes, `vm` perf counters/fast paths, benchmark adapters/schema, and CI gate scripts, with differential semantic validation between optimized and non-optimized execution.

**Major components:**
1. Benchmark & instrumentation control plane (`crates/benchmarks`, schema, adapters, reporters) — reproducible evidence generation.
2. Engine optimization plane (`crates/bytecode/src/opt`, `crates/vm/src/fast_path`, `crates/vm/src/perf`) — guarded semantics-preserving acceleration.
3. Governance gate layer (`perf_gate.py`, CI job ordering, baseline policy docs) — enforce thresholds and prevent correctness regressions.

### Critical Pitfalls

1. **Apples-to-oranges cross-engine benchmarking** — define and enforce compile/execute fairness contract + parity checks before tuning.
2. **Noise mistaken for speedup** — require environment metadata, minimum sample policy, and variance-aware thresholds.
3. **Semantic drift from fast paths** — enforce guard/deopt design and optimization on/off differential semantic tests.
4. **Aggregate-only KPI bias** — gate both aggregate target and per-case non-regression bands.
5. **Brittle or loose CI thresholds / baseline drift** — pin comparator versions and manage threshold policy lifecycle (owner, rationale, expiry, reset procedure).

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 10: Baseline Contract & Benchmark Normalization
**Rationale:** No optimization evidence is trustworthy until benchmark fairness and reproducibility are standardized.
**Delivers:** Versioned benchmark schema, case catalog, engine adapters parity, pinned comparator metadata, baseline snapshots (`local-dev`, `ci-linux`).
**Addresses:** PERF-01, PERF-02.
**Avoids:** Pitfall 1, Pitfall 2, Pitfall 9.

### Phase 11: Instrumentation + Semantic Parity Harness
**Rationale:** Hotspot attribution and semantic safety must exist before introducing aggressive optimizations.
**Delivers:** VM perf counters/snapshots, bytecode pass framework, optimization on/off differential test harness, hotspot evidence format.
**Uses:** `tracing`/`pprof` (diagnostic), pass budgeting/feature flags.
**Implements:** Architecture control-plane to execution-plane observability bridge.

### Phase 12: Targeted Hot-Path Optimization Wave
**Rationale:** After measurement and safety rails are in place, optimize one workload family at a time for clear attribution/rollback.
**Delivers:** At least two measured optimizations across arithmetic loops, array operations, and call-heavy paths; per-case + aggregate delta reports; allocation/GC side-effect checks.
**Uses:** `criterion`, `iai-callgrind`, and optional `smallvec`/`rustc-hash`/`bumpalo` only when profile-backed.
**Implements:** Guarded fast-path + canonical fallback architecture pattern.

### Phase 13: CI Perf Governance & Release Gate Hardening
**Rationale:** Convert milestone wins into durable regression protection without introducing flaky gates.
**Delivers:** `perf_gate.py` threshold policy (PR coarse + nightly strict), artifact publication, baseline reset protocol, merge rule requiring both perf and semantic evidence.
**Addresses:** TST-05, TST-06.
**Avoids:** Pitfall 8, Pitfall 10.

### Phase Ordering Rationale

- Benchmark contract and metadata pinning come first because all later optimization decisions depend on trustworthy evidence.
- Instrumentation/parity harness precedes optimization to prevent semantic regressions and to improve causal confidence of each speedup.
- CI gating comes after initial stabilization so thresholds can be calibrated from real variance data, reducing flake risk.
- This ordering preserves layered architecture ownership and avoids cross-layer optimization debt.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 10:** Cross-platform comparator execution contract (especially `quickjs-c` path/WSL behavior) and compile-vs-execute equivalence details.
- **Phase 12:** Cache invalidation and mutation-heavy semantics for any property/call caching fast paths.
- **Phase 13:** Threshold calibration windows, rerun policy, and baseline reset governance tuned from observed CI variance.

Phases with standard patterns (skip research-phase):
- **Phase 11:** Rust instrumentation modules, pass manager scaffolding, and differential testing are established implementation patterns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Versions and tool fit are validated; integration path aligns with existing repo structure. |
| Features | HIGH | Must/should/defer boundaries are consistent with v1.1 goal and constraints. |
| Architecture | HIGH | Layered control-plane approach matches current system and minimizes blast radius. |
| Pitfalls | HIGH | Risks are concrete, recurring in perf milestones, and mapped to prevention phases. |

**Overall confidence:** HIGH (execution risk remains in hotspot discovery quality and CI variance calibration).

### Gaps to Address

- **Hotspot ranking is not yet empirical:** finalize priority only after Phase 10/11 instrumentation data on current baseline.
- **Threshold numbers are not finalized:** derive from rolling benchmark history, not one-off runs.
- **GC/allocation telemetry contract is incomplete:** define required counters and reporting schema before broad optimization rollout.
- **Comparator availability in CI (especially quickjs-c pathing):** validate runner setup and fallback behavior early.

## Sources

### Primary (HIGH confidence)
- `.planning/research/STACK.md` — tooling/version recommendations and stack constraints.
- `.planning/research/FEATURES.md` — must/should/defer feature groups and dependency chain.
- `.planning/research/ARCHITECTURE.md` — component model, data flow, and build-order rationale.
- `.planning/research/PITFALLS.md` — failure modes, warning signs, and phase-level mitigations.
- `.planning/PROJECT.md` — milestone scope, goals, and non-negotiable constraints.

### Secondary (MEDIUM confidence)
- `docs/engine-benchmarks.md` and `docs/reports/engine-benchmark-report.md` — current benchmark process and baseline evidence shape.
- `.github/workflows/ci.yml`, `crates/benchmarks/src/main.rs` — existing implementation baseline and integration reality.

### Tertiary (LOW confidence)
- Inferred phase naming alignment (Phase 10-13) based on research documents; exact numbering should be confirmed during roadmap drafting.

---
*Research completed: 2026-02-27*
*Ready for roadmap: yes*