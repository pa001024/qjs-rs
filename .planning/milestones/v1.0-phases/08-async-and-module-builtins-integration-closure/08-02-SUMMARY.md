---
phase: 08-async-and-module-builtins-integration-closure
plan: 02
subsystem: async-runtime
tags: [module, promise, queue, host-hooks, harness]
requires:
  - phase: 08-async-and-module-builtins-integration-closure
    provides: module realm builtin parity and deterministic lifecycle error mapping from 08-01
provides:
  - module-evaluated Promise reactions now enqueue onto the shared VM promise job queue
  - exact-name VM regressions proving module-path queue ordering and host-hook drain contracts
  - harness module async matrix and reusable module-eval drain helper for host-driven assertions
affects: [08-03-PLAN.md, verification-traceability, async-module-regressions]
tech-stack:
  added: []
  patterns:
    - module async evidence must drain via explicit host hooks, never implicit auto-drain
key-files:
  created:
    - crates/test-harness/tests/module_async_integration.rs
  modified:
    - crates/vm/src/lib.rs
    - crates/vm/tests/module_lifecycle.rs
    - crates/test-harness/src/lib.rs
    - crates/test-harness/tests/promise_job_queue.rs
key-decisions:
  - Execute module chunks on the active VM and seed module scope from realm globals so module-originated Promise jobs stay visible to shared host hooks.
  - Validate module async behavior through host-driven drain reports/events instead of relying on synchronous export snapshots.
  - Introduce `run_module_entry_with_vm` helper to standardize module evaluation + explicit queue draining across harness async tests.
patterns-established:
  - Module async regressions should assert queue stop reasons (`BudgetExhausted`/`QueueEmpty`) in addition to event order.
  - Callback failure tokens must be asserted under module-originated jobs for enqueue/drain-start/drain-end hooks.
requirements-completed: [ASY-01, ASY-02]
duration: 5 min
completed: 2026-02-27
---

# Phase 08 Plan 02: Add module-path Promise queue regression matrix Summary

**Module-executed Promise chains now flow through the shared VM queue with deterministic host-hook observability, backed by VM and harness module-async regressions.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-27T18:31:48+08:00
- **Completed:** 2026-02-27T18:36:34+08:00
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Reworked module record execution to run on the active `Vm` and preserve module-originated Promise jobs in the shared queue.
- Added exact-name VM module lifecycle tests for queue semantics and host-hook drain/failure visibility.
- Added harness module async matrix plus reusable helper flow for module evaluation + host-driven draining.

## Task Commits

Each task was committed atomically:

1. **Task 1: Ensure module-evaluated Promise reactions flow through shared VM queue and host hooks** - `9f1137f` (fix)
2. **Task 2: Add harness module-async integration matrix for `then/catch/finally` ordering and callback contracts** - `7b3694f` (test)
3. **Task 3: Add reusable harness helper flow for module evaluation + host-driven drain assertions** - `efffd45` (feat)

**Plan metadata:** `(pending)`

## Files Created/Modified
- `crates/vm/src/lib.rs` - Routed module chunk execution through shared VM state and seeded module execution bindings from realm globals.
- `crates/vm/tests/module_lifecycle.rs` - Added exact-name module queue semantics and host-hook drain contract regressions.
- `crates/test-harness/src/lib.rs` - Added `run_module_entry_with_vm` + `ModuleEntryExecution` helper for explicit host-driven drains.
- `crates/test-harness/tests/promise_job_queue.rs` - Added exact-name `module_path_promise_queue_matrix` regression for module-originated jobs.
- `crates/test-harness/tests/module_async_integration.rs` - Added module async ordering/host visibility regressions using shared helper contract.

## Decisions Made
- Keep module Promise queue visibility on the same VM hook surface instead of introducing a module-specific hook API.
- Make module async assertions drain-driven so ASY-01/ASY-02 evidence is tied to explicit host control.
- Preserve helper-level drain realm as harness-managed default to avoid hidden auto-drain behavior in tests.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Initial module async fixtures produced zero pending jobs due synchronous constructor settlement**
- **Found during:** Task 2 (module async matrix validation)
- **Issue:** Constructor-based fixture variants could settle fully during module evaluation and failed to prove queue behavior.
- **Fix:** Switched matrix fixtures to async-function-rooted chains that deterministically leave observable queued jobs for host-driven drains.
- **Files modified:** `crates/vm/tests/module_lifecycle.rs`, `crates/test-harness/tests/module_async_integration.rs`, `crates/test-harness/tests/promise_job_queue.rs`
- **Verification:** Re-ran VM exact-name and harness module async/promise queue exact-name commands; all passed.
- **Committed in:** `9f1137f`, `7b3694f` (task commits)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** No scope creep; fixture correction was required to produce deterministic module-path queue evidence.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Module-path ASY-01/ASY-02 evidence is now explicit in VM + harness regressions.
- Ready for `08-03-PLAN.md` CI/wiring pass to freeze phase-level gates and verification artifacts.

---
*Phase: 08-async-and-module-builtins-integration-closure*
*Completed: 2026-02-27*
