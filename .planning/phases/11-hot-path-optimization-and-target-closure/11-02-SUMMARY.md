---
phase: 11-hot-path-optimization-and-target-closure
plan: 02
subsystem: performance
tags: [vm, fast-path, benchmark, packet-a, perf-target]
requires:
  - phase: 11-hot-path-optimization-and-target-closure
    provides: [phase-11 baseline artifact, closure policy, hotspot attribution scaffolding]
provides:
  - guarded numeric packet-A fast paths for Add/Sub/Mul/Div and primitive number coercion
  - guarded binding-resolution cache path with deterministic invalidation hooks and fallback safety
  - packet-A benchmark artifact and validated baseline delta evidence for target cases
affects: [phase-11-packet-b, perf-closure-audit, vm-hot-path-maintenance]
tech-stack:
  added: []
  patterns: [guarded-fast-path-with-fallback, explicit-scope-cache-invalidation, packet-evidence-gating]
key-files:
  created:
    - crates/vm/src/fast_path.rs
    - crates/vm/tests/perf_packet_a.rs
    - crates/benchmarks/tests/perf_packet_a_report.rs
    - .planning/phases/11-hot-path-optimization-and-target-closure/11-PACKET-A-EVIDENCE.md
  modified:
    - crates/vm/src/lib.rs
    - crates/benchmarks/src/main.rs
key-decisions:
  - Packet-A numeric acceleration remains default-on, while binding cache guard counters are explicitly metrics-gated to avoid benchmark-path overhead.
  - Binding cache safety is enforced by deterministic invalidation on scope-stack mutations, `with` interactions, and global/property fallthrough outcomes.
  - Packet-A evidence is accepted only after contract validation and explicit per-case perf-target checks against the locked Phase 11 baseline.
patterns-established:
  - Optimization packets can ship with parity tests + checker-validated artifact deltas in the same plan.
  - VM hot-path counters can be made observable in tests without forcing overhead in production benchmark paths.
requirements-completed:
  - PERF-03
  - PERF-04
  - PERF-05
duration: 31 min
completed: 2026-02-28
---

# Phase 11 Plan 02: Packet-A numeric/binding optimization Summary

**Packet-A landed guarded numeric + binding fast paths with fallback safety, plus a checker-validated `packet-a` artifact that improves required hotspots against the locked Phase 11 baseline.**

## Performance

- **Duration:** 31 min
- **Started:** 2026-02-28T06:25:00Z
- **Completed:** 2026-02-28T06:56:00Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `crates/vm/src/fast_path.rs` and integrated packet-A VM dispatch wiring in `crates/vm/src/lib.rs`:
  - guarded numeric fast paths for `Add`, `Sub`, `Mul`, `Div`
  - guarded primitive number coercion shortcut
  - deterministic fallback to canonical coercion/property logic when guards miss
- Added guarded binding-resolution acceleration with invalidation and fallback guarantees:
  - cache invalidation on scope-stack mutations and `with` entry/exit
  - cache eviction on property/global fallthrough paths
  - scoped parity/invalidation coverage in `crates/vm/tests/perf_packet_a.rs`
- Added packet-A benchmark tagging/evidence flow:
  - `perf_target` metadata builder/tagging path in `crates/benchmarks/src/main.rs`
  - packet-A report tests in `crates/benchmarks/tests/perf_packet_a_report.rs`
  - audited evidence log in `11-PACKET-A-EVIDENCE.md`

## Task Commits

1. **Task 1: Guarded numeric fast paths + binding accelerator core** — `ecefdbb` (perf)  
   Follow-up tuning for benchmark-path overhead: `a8a730b` (perf)
2. **Task 2: Binding invalidation/fallback tests** — `e88c475` (test)
3. **Task 3: Packet-A artifact tagging + evidence publication** — `d4298d9` (feat)

## Verification

- `cargo test -p vm packet_a_numeric_fast_path_parity -- --exact` ✅
- `cargo test -p vm packet_a_binding_cache_scope_invalidation -- --exact` ✅
- `cargo test -p vm packet_a_binding_cache_with_scope_fallback -- --exact` ✅
- `cargo test -p benchmarks perf_packet_a_report -- --nocapture` ✅
- `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-a.json --allow-missing-comparators` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-a.json` ✅
- `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-a.json --expect-case-improvement arith-loop --expect-case-improvement fib-iterative --max-case-regression json-roundtrip=1.10` ✅

## Deviations from Plan

- **[Rule 1 - Bug] Packet-A initial guard accounting/regression overhead**
  - Found during: Task 3 evidence run
  - Issue: initial packet-A candidate failed perf-target improvement checks for `arith-loop`/`fib-iterative`.
  - Fix: reduced guard overhead (HashMap cache + metrics-gated counter recording on benchmark path) while preserving parity/fallback tests.
  - Files modified: `crates/vm/src/fast_path.rs`, `crates/vm/src/lib.rs`, `crates/vm/tests/perf_packet_a.rs`
  - Verification: packet-A VM tests + perf-target checker pass
  - Commit: `a8a730b`

Total deviations: 1 auto-fixed (Rule 1).

## Issues Encountered

None.

## User Setup Required

None.

## Next Phase Readiness

- Packet-A hotspot evidence and safety invariants are now in place.
- Ready for `11-03-PLAN.md` (array/property packet + closure rerun).

## Self-Check: PASSED
