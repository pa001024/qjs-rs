# Roadmap: qjs-rs

## Overview

v1.1 (`Performance Acceleration`) is focused on measurable runtime speed improvements with strict semantic non-regression. Phase scope is derived only from active v1.1 requirements and starts at **Phase 10** (v1.0 ended at Phase 9).

## Milestones

- ✅ **v1.0 milestone** — Phases 1-9 shipped on 2026-02-27. Archive: `.planning/milestones/v1.0-ROADMAP.md`
- 🚧 **v1.1 milestone** — Performance Acceleration execution in progress (Plan 10-01 completed on 2026-02-27)

## Phases

- [ ] **Phase 10: Baseline Contract and Benchmark Normalization** - Establish reproducible cross-engine performance evidence and hot-path benchmark coverage. (`PERF-01`, `PERF-02`)
- [ ] **Phase 11: Hot-Path Optimization and Target Closure** - Land targeted runtime optimizations to reach aggregate `qjs-rs <= boa-engine` while preserving architecture boundaries. (`PERF-03`, `PERF-04`, `PERF-05`)
- [ ] **Phase 12: Performance Governance and Non-Regression Gates** - Enforce correctness + performance regression guardrails in CI/nightly with deterministic artifacts. (`TST-05`, `TST-06`)

## Phase Details

### Phase 10: Baseline Contract and Benchmark Normalization
**Goal**: Benchmark outputs are reproducible, comparable, and representative for v1.1 optimization decisions.
**Depends on**: v1.0 shipped baseline
**Requirements**: PERF-01, PERF-02
**Success Criteria** (what must be TRUE):
  1. Benchmark harness emits machine-readable JSON and human-readable reports for `qjs-rs`, `boa-engine`, `quickjs-c`, and `nodejs`.
  2. Benchmark artifacts capture run metadata (engine versions, host info, run controls) sufficient for reproducibility.
  3. Case suite includes arithmetic loop, iterative function calls, array build/sum, and JSON roundtrip workloads.
  4. Sample/iteration controls are configurable and documented for both local and CI runs.

### Phase 11: Hot-Path Optimization and Target Closure
**Goal**: Achieve competitive aggregate latency versus `boa-engine` through evidence-backed VM/runtime/bytecode optimizations.
**Depends on**: Phase 10
**Requirements**: PERF-03, PERF-04, PERF-05
**Success Criteria** (what must be TRUE):
  1. Aggregate mean latency on the tracked suite is no worse than `boa-engine` under the same host and run configuration.
  2. At least two hot paths (from arithmetic/array/call-heavy families) receive targeted optimizations with before/after evidence.
  3. Each optimization includes guard/fallback behavior and does not alter externally observable semantics.
  4. Runtime-core remains pure Rust (no C FFI introduced) and optimization changes stay within maintainable layer boundaries.

### Phase 12: Performance Governance and Non-Regression Gates
**Goal**: Performance gains become durable through automated gates that preserve semantic correctness.
**Depends on**: Phase 11
**Requirements**: TST-05, TST-06
**Success Criteria** (what must be TRUE):
  1. Governance gates (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, test262-lite checks) remain green after optimization work.
  2. CI/nightly workflows execute documented performance regression checks with explicit thresholds.
  3. Performance artifacts are written to deterministic, documented output paths for audit and trend tracking.
  4. Merge/release guidance documents required perf + semantic evidence for v1.1 changes.

## Coverage Validation

- Active v1.1 requirements: **7**
- Requirements mapped to phases: **7/7 (100%)**
- Unmapped requirements: **0**
- Multi-mapped requirements: **0**

## Progress

| Milestone | Status | Phases | Plans |
|-----------|--------|--------|-------|
| v1.0 | Complete | 9/9 | 26/26 |
| v1.1 | In Progress | 0/3 | 1/3 |
