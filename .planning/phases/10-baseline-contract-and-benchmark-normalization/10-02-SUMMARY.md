---
phase: 10-baseline-contract-and-benchmark-normalization
plan: 02
subsystem: performance
tags: [benchmarking, adapter-normalization, comparator-preflight, reproducibility, perf]
requires:
  - phase: 10-baseline-contract-and-benchmark-normalization/10-01
    provides: benchmark contract envelope, required case catalog, and profile controls
provides:
  - uniform eval-per-iteration timing semantics across all benchmark adapters
  - comparator preflight metadata (command/path/workdir/version/status/reason) serialized for reproducibility
  - deterministic adapter normalization regression tests without live Node/QuickJS process dependencies
affects: [phase-10-plan-03, phase-11-hot-path-optimization, benchmark-governance]
tech-stack:
  added: []
  patterns:
    - External comparator adapters are preflight-validated before running benchmark cases.
    - Adapter normalization policy is enforced through deterministic tests that avoid global environment mutation.
key-files:
  created:
    - crates/benchmarks/Cargo.toml
    - crates/benchmarks/tests/adapter_normalization.rs
  modified:
    - Cargo.toml
    - crates/benchmarks/src/contract.rs
    - crates/benchmarks/src/main.rs
    - docs/benchmark-contract.md
key-decisions:
  - Keep one timing mode (`eval-per-iteration`) for all engines so cross-engine metrics remain comparable.
  - Capture comparator strictness and per-engine preflight metadata in reproducibility artifacts to fail fast in strict profiles.
  - Use env-injected parser helpers in tests instead of mutating process environment variables.
patterns-established:
  - Adapter normalization regressions are guarded with process-free fixture tests under `adapter_normalization`.
  - Comparator command/path/workdir resolution follows explicit CLI > ENV > default precedence.
requirements-completed:
  - PERF-01
  - PERF-02
duration: 4h 22m
completed: 2026-02-28
---

# Phase 10 Plan 02: Adapter normalization and comparator preflight closure Summary

**Benchmark adapters now run under one shared timing contract with reproducibility-grade comparator metadata and deterministic normalization regression tests.**

## Performance

- **Duration:** 4h 22m
- **Started:** 2026-02-27T23:34:35Z
- **Completed:** 2026-02-28T03:57:00Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Enforced adapter parity semantics so all engines use the same `eval-per-iteration` timing mode and value-based checksum normalization.
- Added configurable comparator controls + preflight status metadata serialization for `nodejs` and `quickjs-c` (command/path/workdir/version/status/reason + strict mode).
- Added deterministic `adapter_normalization` regression tests that validate timing-mode parity, CLI/env precedence, reproducibility metadata completeness, and checksum fold behavior without launching external comparators.

## Task Commits

Each task was committed atomically:

1. **Task 1: Enforce adapter timing/checksum parity under one contract timing mode** - `612bee8` (feat)
2. **Task 2: Add comparator preflight and configurable command/path controls** - `377f20b` (feat)
3. **Task 3: Add adapter-normalization regression tests without external-process dependency** - `641b6ff` (test)

**Plan metadata:** captured in the plan-closure documentation commit for `10-02`.

## Files Created/Modified

- `Cargo.toml` - Registers `crates/benchmarks` in workspace so `cargo test -p benchmarks` is executable.
- `crates/benchmarks/Cargo.toml` - Adds benchmark crate manifest and workspace-linked dependencies.
- `crates/benchmarks/src/contract.rs` - Adds env-injected CLI parsing path used by deterministic normalization tests.
- `crates/benchmarks/src/main.rs` - Exposes adapter policy helpers for regression assertions.
- `crates/benchmarks/tests/adapter_normalization.rs` - Process-free adapter normalization regression suite.
- `docs/benchmark-contract.md` - Documents comparator control precedence and preflight metadata requirements.

## Decisions Made

- Preserve a single timing contract (`eval-per-iteration`) for all adapters to avoid apples-to-oranges comparison drift.
- Treat comparator availability/version checks as first-class reproducibility evidence in artifact metadata.
- Keep regression tests deterministic by injecting env values via helper APIs instead of using unsafe global env mutation.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Benchmarks crate was not workspace-addressable for plan verification commands**
- **Found during:** Task 3 verification (`cargo test -p benchmarks ...`)
- **Issue:** `crates/benchmarks` lacked a committed crate manifest/workspace registration, blocking package-targeted verification.
- **Fix:** Added `crates/benchmarks/Cargo.toml` and workspace member entry in root `Cargo.toml`.
- **Files modified:** `Cargo.toml`, `crates/benchmarks/Cargo.toml`
- **Verification:** `cargo test -p benchmarks` executed successfully.
- **Committed in:** `641b6ff`

**2. [Rule 1 - Bug] Adapter normalization tests failed under Rust 2024 due to global env mutation API safety gate**
- **Found during:** Task 3 verification (compile failure E0133 on `std::env::set_var/remove_var`)
- **Issue:** Original test approach mutated process env directly, causing unsafe-call compile errors and nondeterministic coupling.
- **Fix:** Switched tests to `parse_cli_args_with_env` fixture injection, removing runtime env mutation.
- **Files modified:** `crates/benchmarks/src/contract.rs`, `crates/benchmarks/tests/adapter_normalization.rs`
- **Verification:** `cargo test -p benchmarks adapter_normalization -- --nocapture` passed.
- **Committed in:** `641b6ff`

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes were required for deterministic and executable verification; no scope creep beyond plan intent.

## Issues Encountered

- Large warning volume appears when integration tests include `src/main.rs` via `#[path = ...]`; warnings are non-blocking and tests remain deterministic/passing.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Adapter semantics and comparator reproducibility evidence are normalized for baseline comparison.
- Ready for `10-03-PLAN.md` execution.

---
*Phase: 10-baseline-contract-and-benchmark-normalization*
*Completed: 2026-02-28*
