---
phase: 10-baseline-contract-and-benchmark-normalization
plan: 03
subsystem: infra
tags: [benchmark, perf, ci, reproducibility, contract]
requires:
  - phase: 10-02
    provides: [normalized timing mode, comparator preflight metadata]
provides:
  - benchmark JSON contract checker with deterministic self-test fixtures
  - markdown report rendering with schema/profile/timing/comparator metadata context
  - reproducible local/ci benchmark runbook and CI contract gate wiring
affects: [phase-11-hot-path-optimization, perf-governance, benchmark-evidence]
tech-stack:
  added: [python]
  patterns: [fixture-backed contract validation, deterministic benchmark artifact paths]
key-files:
  created:
    - .github/scripts/check_engine_benchmark_contract.py
    - .github/scripts/benchmark_contract/fixtures/benchmark-report-valid.json
    - .github/scripts/benchmark_contract/fixtures/benchmark-report-missing-case.json
    - scripts/render_engine_benchmark_report.py
    - docs/engine-benchmarks.md
    - .planning/phases/10-baseline-contract-and-benchmark-normalization/10-BASELINE-CONTRACT-EVIDENCE.md
  modified:
    - .github/workflows/ci.yml
key-decisions:
  - Validate benchmark artifacts against bench.v1 contract before report rendering/publishing.
  - Keep CI contract gate deterministic by using fixture-backed fast checks (`--self-test` + valid fixture input).
  - Render comparator availability metadata directly in markdown and mark unavailable engines as `N/A` in latency tables.
patterns-established:
  - Benchmark evidence flow is fixed to run -> contract-check -> render for both local-dev and ci-linux profiles.
  - Deterministic output paths (`target/benchmarks/engine-comparison.<profile>.*`) are now part of runbook and evidence policy.
requirements-completed:
  - PERF-01
  - PERF-02
duration: 18 min
completed: 2026-02-28
---

# Phase 10 Plan 03: Baseline contract closure and benchmark normalization Summary

**Phase 10 now enforces a deterministic benchmark evidence contract from JSON validation through metadata-rich report publication in both local and CI workflows.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-02-28T04:07:00Z
- **Completed:** 2026-02-28T04:25:00Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `.github/scripts/check_engine_benchmark_contract.py` to enforce bench.v1 schema, metadata completeness, required engines, and required case coverage with deterministic fixture-backed self-tests.
- Extended `scripts/render_engine_benchmark_report.py` so markdown output includes schema/profile/timing/run-control metadata, comparator status/version/path details, and clear `N/A (status)` markers for unavailable engines.
- Updated `docs/engine-benchmarks.md`, `.github/workflows/ci.yml`, and `10-BASELINE-CONTRACT-EVIDENCE.md` to standardize reproducible command ordering and deterministic artifact paths for `local-dev` and `ci-linux`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement benchmark contract checker with deterministic fixtures/self-test** - `63abdce` (feat)
2. **Task 2: Update human-readable report renderer to surface contract metadata** - `4721ede` (feat)
3. **Task 3: Wire CI/runbook contract checks and record Phase 10 evidence procedure** - `c39dd6d` (docs)

**Plan metadata:** captured in the plan-closure documentation commit for `10-03`.

## Files Created/Modified

- `.github/scripts/check_engine_benchmark_contract.py` - Contract checker for benchmark JSON artifacts plus fixture-backed self-test mode.
- `.github/scripts/benchmark_contract/fixtures/benchmark-report-valid.json` - Deterministic valid bench.v1 fixture for CI/self-test.
- `.github/scripts/benchmark_contract/fixtures/benchmark-report-missing-case.json` - Deterministic invalid fixture proving required-case failure behavior.
- `scripts/render_engine_benchmark_report.py` - Metadata-aware report renderer with unavailable comparator handling.
- `docs/engine-benchmarks.md` - Local/CI reproducibility runbook with deterministic paths and checker invocation sequence.
- `.github/workflows/ci.yml` - Fast benchmark contract gate (`--self-test` + valid fixture check).
- `.planning/phases/10-baseline-contract-and-benchmark-normalization/10-BASELINE-CONTRACT-EVIDENCE.md` - Evidence publication procedure for Phase 10 closure.

## Decisions Made

- Enforce contract validation as a required precondition for benchmark evidence publication.
- Keep CI gate lightweight/deterministic with fixture validation instead of full benchmark execution.
- Surface comparator availability directly in human-readable reports so reviewers can distinguish real latency values from unavailable engines.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 10 plan set is complete with contract checker, metadata-rich reporting, and deterministic runbook/CI gating.
- Ready for Phase 10 closure transition and Phase 11 optimization execution.

---
*Phase: 10-baseline-contract-and-benchmark-normalization*
*Completed: 2026-02-28*
