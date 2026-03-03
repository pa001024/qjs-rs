---
phase: 03-promise-job-queue-semantics
plan: 01
subsystem: runtime
tags: [promise, microtask, queue, host-hooks, gc-roots]

requires:
  - phase: 02-runtime-safety-and-root-integrity
    provides: pending-job root candidate lifecycle and deterministic GC root collection
provides:
  - Promise prototype native dispatch surface for then/catch/finally
  - VM-owned FIFO Promise job queue with bounded drain report semantics
  - host callback contract for enqueue and drain lifecycle hooks
affects: [phase-03-promise-job-queue-semantics, phase-04-es-module-lifecycle]

tech-stack:
  added: []
  patterns: [single-vm-promise-job-queue, callback-guarded-host-hooks, queue-root-handle-capture]

key-files:
  created: [.planning/phases/03-promise-job-queue-semantics/03-01-SUMMARY.md]
  modified: [crates/runtime/src/lib.rs, crates/vm/src/lib.rs]

key-decisions:
  - "Promise.prototype.then/catch/finally are native dispatch targets and receive receiver through execute_callable pre-injection."
  - "Queue captures always register pending-job root candidate handles and release exactly once on job consumption."

patterns-established:
  - "Host hook guard pattern: enqueue/drain hooks are mandatory entry points and callback failures map to fixed TypeError tokens."
  - "Drain report pattern: every bounded drain returns processed/remaining plus deterministic stop reason."

requirements-completed: [ASY-02]
duration: 55 min
completed: 2026-02-26
---

# Phase 3 Plan 01: Promise Queue Contract Summary

**VM now owns a deterministic FIFO Promise job queue with callback-guarded host enqueue/drain APIs and Promise prototype dispatch routed to queue-backed execution.**

## Performance

- **Duration:** 55 min
- **Started:** 2026-02-26T08:00:00Z
- **Completed:** 2026-02-26T08:55:00Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- Added `NativeFunction::PromiseThen/PromiseCatch/PromiseFinally` and bound them on `Promise.prototype`.
- Implemented VM queue core (`PromiseJobQueue`) with FIFO, bounded drain, and stop-reason reporting.
- Added required host hook contract (`on_enqueue`, `on_drain_start`, `on_drain_end`) with deterministic misuse/type-error mapping.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Promise-native dispatch surface for queue-backed prototype methods** - `50d751b` (feat)
2. **Task 2: Introduce VM Promise job queue core with FIFO and bounded drain** - `50d751b` (feat)
3. **Task 3: Expose required host callback enqueue + drain contract and lock misuse failures** - `50d751b` (feat)

**Plan metadata:** pending

## Files Created/Modified
- `.planning/phases/03-promise-job-queue-semantics/03-01-SUMMARY.md` - Plan 03-01 execution record.
- `crates/runtime/src/lib.rs` - Promise native function surface extension.
- `crates/vm/src/lib.rs` - Promise queue, host hook contract, queue drain/report semantics.

## Decisions Made
- Reused pending-job root candidate infrastructure for queue capture rooting to preserve MEM-01 invariants.
- Kept queue ownership fully inside `Vm` and exposed only callback-guarded host APIs (no direct queue mutation surface).

## Deviations from Plan

- Consolidated queue-core and host-contract implementation into one cohesive engine commit to avoid partial intermediate semantics across shared VM sections.

## Issues Encountered
- None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Queue contract and host hooks are stable for propagation wiring in 03-02.

---
*Phase: 03-promise-job-queue-semantics*
*Completed: 2026-02-26*
