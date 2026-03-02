# Roadmap: qjs-rs

## Overview

v1.1 (`Performance Acceleration`) is focused on measurable runtime speed improvements with strict semantic non-regression. Phase scope is derived only from active v1.1 requirements and starts at **Phase 10** (v1.0 ended at Phase 9).

## Milestones

- ✅ **v1.0 milestone** — Phases 1-9 shipped on 2026-02-27. Archive: `.planning/milestones/v1.0-ROADMAP.md`
- 🚧 **v1.1 milestone** — Performance Acceleration execution in progress (Phase 10 completed on 2026-02-28)

## Phases

- [x] **Phase 10: Baseline Contract and Benchmark Normalization** - Establish reproducible cross-engine performance evidence and hot-path benchmark coverage. (`PERF-01`, `PERF-02`) (completed 2026-02-28)
- [ ] **Phase 11: Hot-Path Optimization and Target Closure** - Land targeted runtime optimizations to reach **>=80% of `quickjs-c` performance** on the tracked suite (latency-equivalent gate: `qjs-rs <= 1.25x quickjs-c`) while preserving architecture boundaries. (`PERF-03`, `PERF-04`, `PERF-05`) (all Phase 11 plans executed as of 2026-02-28; closure remains open per latest `phase11-closure-bundle.json` PERF-03 failure)
- [ ] **Phase 12: Performance Governance and Non-Regression Gates** - Enforce correctness + performance regression guardrails in CI/nightly with deterministic artifacts. (`TST-05`, `TST-06`) (blocked until Phase 11 gap queue closes)

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
**Goal**: Achieve >=80% of `quickjs-c` performance on the tracked suite through evidence-backed VM/runtime/bytecode optimizations.
**Depends on**: Phase 10
**Requirements**: PERF-03, PERF-04, PERF-05
**Plans:** 7/7 plans completed (closure still open: latest authoritative bundle has PERF-03 red gate)
Plans:
- [x] 11-01-PLAN.md — Lock closure policy, add hotspot attribution, and produce Phase 11 baseline artifact. (completed 2026-02-28)
- [x] 11-02-PLAN.md — Land packet-A numeric/binding optimizations with guarded fallback and before/after evidence. (completed 2026-02-28)
- [x] 11-03-PLAN.md — Land packet-B array/property optimizations and run final target-closure evidence bundle. (completed 2026-02-28; see `11-TARGET-CLOSURE-EVIDENCE.md`)
- [x] 11-04-PLAN.md — Land packet-C identifier/global lookup fast path, parity suite, and closure rerun evidence bundle. (completed 2026-02-28; see `11-PACKET-C-EVIDENCE.md`)
- [x] 11-05-PLAN.md — Close governance/test debt and rerun authoritative closure bundle with packet-c artifact refresh. (completed 2026-02-28; closure remains open, see `11-TARGET-CLOSURE-EVIDENCE.md`)
- [x] 11-06-PLAN.md — Build packet-D identifier-slot cache closure candidate and generate packet-d evidence for PERF-03 rerun. (completed 2026-02-28; PERF-03 still open, see `11-PACKET-D-EVIDENCE.md`)
- [x] 11-07-PLAN.md — Execute final governance + PERF-03 authoritative bundle and synchronize traceability docs from single-run provenance. (completed 2026-02-28; bundle remained red so phase closure stays open)
**Success Criteria** (what must be TRUE):
  1. Aggregate mean latency on the tracked suite is at most `1.25x quickjs-c` under the same host and run configuration (equivalent to >=80% of `quickjs-c` performance).
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
| v1.1 | In Progress | 1/3 | 10/10 |
