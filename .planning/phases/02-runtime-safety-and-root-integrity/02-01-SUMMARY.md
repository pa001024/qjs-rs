---
phase: 02-runtime-safety-and-root-integrity
plan: 01
subsystem: runtime
tags: [gc, roots, vm, memory-safety]

requires:
  - phase: 01-semantic-core-closure
    provides: vm/runtime baseline with deterministic GC entry points
provides:
  - module-cache candidate roots are first-class VM GC roots
  - pending-job candidate roots are first-class VM GC roots
  - regression coverage for survival/reclamation and runtime/boundary GC determinism
affects: [phase-03-promise-job-queue-semantics, phase-04-es-module-lifecycle]

tech-stack:
  added: []
  patterns: [vm-internal-root-candidate-buckets, deterministic-root-snapshot-testing]

key-files:
  created: [.planning/phases/02-runtime-safety-and-root-integrity/02-01-SUMMARY.md]
  modified: [crates/vm/src/lib.rs]

key-decisions:
  - "Keep module/job root registration internal to Vm and avoid host-facing APIs in this phase."
  - "Wire both boundary GC and runtime GC through the same collect_roots snapshot path."

patterns-established:
  - "Root category expansion pattern: new reference buckets must be appended in collect_roots and covered by survival+reclamation tests."
  - "Lifecycle reset pattern: execute_in_realm must clear phase-local root candidate state to prevent cross-run leakage."

requirements-completed: [MEM-01]
duration: 46 min
completed: 2026-02-26
---

# Phase 2 Plan 01: Runtime Root Candidate Coverage Summary

**VM GC now treats module-cache and pending-job candidate references as first-class roots with deterministic survival/reclamation behavior across boundary and runtime collection paths.**

## Performance

- **Duration:** 46 min
- **Started:** 2026-02-26T03:34:00Z
- **Completed:** 2026-02-26T04:20:15Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Added VM-owned module-cache/pending-job root candidate buckets with internal register/release/clear helpers.
- Reset root candidate buckets in `execute_in_realm` to prevent root leakage across runs.
- Extended `collect_roots` and added focused regressions proving candidate-only reachability survives GC and is reclaimed after release.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add explicit module/job root candidate buckets to VM state** - `ca042c1` (feat)
2. **Task 2: Wire candidate buckets into root traversal and lock with regression tests** - `51d6f4f` (fix)

**Plan metadata:** (pending in this summary commit)

## Files Created/Modified
- `.planning/phases/02-runtime-safety-and-root-integrity/02-01-SUMMARY.md` - plan execution record and handoff metadata.
- `crates/vm/src/lib.rs` - VM root candidate state, root traversal wiring, and MEM-01 regression coverage.

## Decisions Made
- Kept candidate-root registration internal to `Vm` for Phase 2 to avoid premature public async/module API exposure.
- Reused the existing `collect_roots` path for both `collect_garbage` and runtime-triggered collection to keep root sources deterministic.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `cargo test -p vm` initially hit an incremental lock-file access conflict in shared build artifacts; resolved by running with isolated `CARGO_TARGET_DIR` and `CARGO_INCREMENTAL=0`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- MEM-01 root-category coverage is now locked in VM code and tests.
- Phase 2 can continue on MEM-02 (`02-03-PLAN.md`) with no open blocker from this plan.

---
*Phase: 02-runtime-safety-and-root-integrity*
*Completed: 2026-02-26*
