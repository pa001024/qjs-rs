---
phase: 10-baseline-contract-and-benchmark-normalization
plan: 01
subsystem: performance
tags: [benchmarking, contract, reproducibility, schema, perf]
requires:
  - phase: v1.0-shipped-baseline
    provides: benchmark harness foundation and cross-engine runner skeleton
provides:
  - benchmark contract specification (`bench.v1`) with locked engine/case catalogs and profile policy
  - code-owned contract module that defines schema/run-profile/timing/output controls for benchmark artifacts
  - deterministic regression tests that fail on required case-ID and report-envelope drift
affects: [phase-10-plan-02, benchmark-adapter-normalization, benchmark-reporting]
tech-stack:
  added: []
  patterns:
    - Benchmark output shape and required case catalog are owned by `crates/benchmarks/src/contract.rs` instead of ad-hoc inline structs.
    - Run controls (profile/iterations/samples/warmup/output) are contract-first and serialized in report metadata.
key-files:
  created:
    - docs/benchmark-contract.md
    - crates/benchmarks/src/contract.rs
    - crates/benchmarks/tests/benchmark_contract.rs
  modified:
    - crates/benchmarks/src/main.rs
key-decisions:
  - Establish `bench.v1` as explicit benchmark schema version and require it in every artifact envelope.
  - Lock PERF-02 hot-path IDs (`arith-loop`, `fib-iterative`, `array-sum`, `json-roundtrip`) in a contract-owned case catalog.
  - Standardize profile-driven artifact naming as `target/benchmarks/engine-comparison.<profile>.json` while still allowing explicit `--output` overrides.
patterns-established:
  - Contract drift must be caught via deterministic serialization tests before long benchmark runs.
  - Benchmark CLI/report wiring should consume contract definitions directly rather than duplicate envelope structs.
requirements-completed:
  - PERF-01
  - PERF-02
duration: 6 min
completed: 2026-02-27
---

# Phase 10 Plan 01: Baseline contract specification and benchmark envelope lock Summary

**A versioned `bench.v1` benchmark contract now governs schema fields, required engines/cases, run profiles, timing mode, and artifact naming through shared docs + code + drift tests.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-27T23:18:47Z
- **Completed:** 2026-02-27T23:24:50Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Authored `docs/benchmark-contract.md` with explicit contract rules for schema versioning, required engine set, required hot-path case IDs, run profiles, timing mode, reproducibility metadata, and missing/unsupported engine policy.
- Added `crates/benchmarks/src/contract.rs` and refactored `crates/benchmarks/src/main.rs` to use contract-owned types/constants for CLI parsing, profile defaults, metadata envelope serialization, and case catalog ownership.
- Added `crates/benchmarks/tests/benchmark_contract.rs` to fail fast on required case-ID drift and benchmark report envelope drift without launching external engine processes.

## Task Commits

Each task was committed atomically:

1. **Task 1: Author benchmark contract specification and artifact policy** - `614e161` (docs)
2. **Task 2: Implement versioned contract module and wire runner/report envelope** - `22cd59b` (feat)
3. **Task 3: Add contract regression tests for schema envelope and required case catalog** - `81cd995` (test)

**Plan metadata:** `(pending)`

## Files Created/Modified

- `docs/benchmark-contract.md` - Canonical `bench.v1` benchmark contract and artifact policy.
- `crates/benchmarks/src/contract.rs` - Contract-owned schema/version/profile/timing/output/reproducibility/case-catalog definitions.
- `crates/benchmarks/src/main.rs` - Benchmark runner refactored to consume contract module for CLI and report serialization.
- `crates/benchmarks/tests/benchmark_contract.rs` - Regression tests for required case IDs and report envelope field guarantees.

## Decisions Made

- `bench.v1` is now the required schema envelope key for benchmark artifacts.
- The required case catalog is centralized in contract code so case ID drift is explicitly test-detected.
- Profile defaults include warmup controls and deterministic output naming, with effective controls embedded in artifact metadata.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 10 adapter/report normalization work can now target a fixed schema envelope and locked required case IDs.
- Ready for `10-02-PLAN.md` execution.

---
*Phase: 10-baseline-contract-and-benchmark-normalization*
*Completed: 2026-02-27*