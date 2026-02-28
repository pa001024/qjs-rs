---
phase: 11-hot-path-optimization-and-target-closure
plan: 01
subsystem: performance
tags: [benchmark, vm, hotspot, perf-target, policy]
requires:
  - phase: 10-baseline-contract-and-benchmark-normalization
    provides: [bench.v1 contract, comparator preflight metadata]
provides:
  - phase-11 perf-target checker with deterministic self-test coverage
  - vm hotspot attribution counters (numeric/identifier/array-index) behind explicit opt-in toggles
  - benchmark report perf-target metadata + optional qjs-rs hotspot attribution snapshot
affects: [phase-11-packet-optimization, perf-evidence-auditing, closure-gating]
tech-stack:
  added: [python]
  patterns: [policy-enforced perf closure, opt-in runtime attribution, artifact metadata contracts]
key-files:
  created:
    - .github/scripts/check_perf_target.py
    - docs/performance-closure-policy.md
    - crates/vm/src/perf.rs
    - crates/vm/tests/perf_hotspot_attribution.rs
    - crates/benchmarks/tests/hot_path_contract.rs
  modified:
    - docs/engine-benchmarks.md
    - crates/vm/src/lib.rs
    - crates/benchmarks/src/contract.rs
    - crates/benchmarks/src/main.rs
    - crates/benchmarks/tests/benchmark_contract.rs
key-decisions:
  - PERF-03 closure is locked to `local-dev` + `eval-per-iteration` + same-host rerun and enforced by `.github/scripts/check_perf_target.py`.
  - Comparator policy requires `qjs-rs` and `boa-engine`; `quickjs-c`/`nodejs` may be unavailable only with explicit status+reason metadata.
  - VM hotspot attribution remains default-off and must be explicitly enabled via VM/API toggle (or benchmark artifact mode inference) to avoid semantic drift.
patterns-established:
  - Phase 11 artifacts now carry machine-checkable `perf_target` metadata and optional `qjs_rs_hotspot_attribution` evidence.
  - Packet evidence flow can prove hotspot-family deltas with deterministic checker gates before closure claims.
requirements-completed:
  - PERF-03
  - PERF-04
  - PERF-05
duration: 11 min
completed: 2026-02-28
---

# Phase 11 Plan 01: Safety/evidence foundation for hot-path optimization Summary

**Phase 11 now has an explicit closure policy, machine-checkable perf-target gate, and hotspot attribution plumbing required for packet-level optimization evidence.**

## Performance

- **Duration:** 11 min
- **Started:** 2026-02-28T06:08:59Z
- **Completed:** 2026-02-28T06:19:50Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments

- Added `.github/scripts/check_perf_target.py` with deterministic self-test fixtures, authoritative profile/timing/same-host enforcement, comparator availability policy checks, and candidate delta options (`--require-qjs-lte-boa`, case improvement/regression gates).
- Added VM attribution primitives via `crates/vm/src/perf.rs` and `crates/vm/src/lib.rs` wiring, covering numeric opcode family, identifier resolution, and indexed property get/set counters with default-off toggles and parity tests.
- Extended benchmark contract/report metadata in `crates/benchmarks/src/contract.rs` + `crates/benchmarks/src/main.rs` to emit `perf_target` metadata and optional `qjs_rs_hotspot_attribution`, plus contract tests and a freshly generated baseline artifact:
  - `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`

## Task Commits

1. **Task 1: Lock PERF-03 closure policy + checker** — `9d8bb68` (feat)
2. **Task 2: VM hotspot attribution primitives + toggle parity tests** — `d8685c0` (feat)
3. **Task 3: Benchmark contract/report metadata + phase11 baseline artifact** — `48a9023` (feat)

## Verification

- `python .github/scripts/check_perf_target.py --self-test` ✅
- `rg --line-number "authoritative|local-dev|qjs-rs|boa-engine|same-host|missing comparator" docs/performance-closure-policy.md docs/engine-benchmarks.md` ✅
- `cargo test -p vm perf_hotspot_attribution_records_opcode_families -- --exact` ✅
- `cargo test -p vm perf_hotspot_toggle_preserves_semantics -- --exact` ✅
- `cargo test -p benchmarks hot_path_contract -- --nocapture` ✅
- `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --allow-missing-comparators` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.phase11-baseline.json` ✅

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None.

## Next Phase Readiness

- `11-01` artifacts establish the Phase 11 closure guardrails and attribution baseline expected by `11-02` packet-A execution.
- Ready for `11-02-PLAN.md`.

## Self-Check: PASSED

