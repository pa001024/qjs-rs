---
phase: 06-collection-and-regexp-semantics
plan: 02
subsystem: runtime
tags: [regexp, builtins, lastindex, syntaxerror]

requires:
  - phase: 06-collection-and-regexp-semantics
    provides: weak-collection constructor/prototype split and collection regression scaffolding
provides:
  - shared RegExp match core for `exec`/`test` with consistent `lastIndex` transitions
  - deterministic RegExp constructor validation and canonical supported-flag normalization
  - harness regression matrix for constructor/exec/test/capture/error semantics
affects: [phase-06-collection-and-regexp-semantics, phase-07-compatibility-and-governance-gates]

tech-stack:
  added: []
  patterns: [shared-regexp-match-core, canonical-regexp-flags, exact-name-vm-gates]

key-files:
  created:
    - .planning/phases/06-collection-and-regexp-semantics/06-02-SUMMARY.md
    - crates/test-harness/tests/regexp_semantics.rs
  modified:
    - crates/vm/src/lib.rs

key-decisions:
  - "Route RegExp.prototype.exec and RegExp.prototype.test through a single VM match helper that also owns `lastIndex` transitions."
  - "Canonicalize supported flags to `gimsuy` before surfacing `flags` and `toString` output to keep constructor state deterministic."
  - "Add exact-name top-level VM tests so plan verification commands using `--exact` always execute concrete tests."

patterns-established:
  - "When plan verification uses `cargo test ... -- --exact`, place at least one matching top-level test name in the vm crate."

requirements-completed: [BUI-05]

duration: 9 min
completed: 2026-02-27
---

# Phase 6 Plan 02: RegExp Constructor and Prototype Semantics Summary

**RegExp constructor validation, exec/test shared matching, and canonical `/source/flags` behavior are now deterministic and guarded by VM plus harness regressions.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-27T04:25:55Z
- **Completed:** 2026-02-27T04:35:29Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- Centralized `RegExp.prototype.exec` and `RegExp.prototype.test` on one match core with shared `lastIndex` contract and writable-guard enforcement.
- Completed constructor semantics for RegExp-input cloning/overrides, deterministic supported-flag/pattern `SyntaxError` failures, and canonical flag output.
- Added harness integration matrix covering constructor behavior, capture-slot materialization, `exec/test` transition alignment, and boundary errors.

## Task Commits

Each task was committed atomically:

1. **Task 1: Centralize RegExp match/lastIndex transitions for exec and test** - `a3c642d` (feat)
2. **Task 2: Complete RegExp constructor, exec result shape, and deterministic SyntaxError boundaries** - `4376142` (feat)
3. **Task 3: Add harness integration matrix for RegExp baseline semantics** - `a7ddcfa` (test)

**Plan metadata:** pending

## Files Created/Modified
- `crates/vm/src/lib.rs` - Added shared RegExp match state path, lastIndex helper, constructor input resolution, canonical flag normalization, and exact-name VM verification tests.
- `crates/test-harness/tests/regexp_semantics.rs` - Added script-level RegExp semantics matrix for clone/flags, exec/test transitions, captures, and deterministic constructor errors.

## Decisions Made
- Consolidated RegExp matching behavior into a single helper to eliminate drift between `exec` and `test` state transitions.
- Canonicalized supported flags to `gimsuy` at slot/surface time to stabilize observable `toString` and `flags` output.
- Added top-level exact-name VM tests to satisfy strict `--exact` verification command contracts.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added top-level exact-name VM tests for plan verify commands**
- **Found during:** Task 1 verification
- **Issue:** `cargo test -p vm regexp_last_index_transition_matrix -- --exact` initially executed zero tests because nested test module names include a path prefix.
- **Fix:** Added top-level exact-name tests (`regexp_last_index_transition_matrix`, `regexp_exec_capture_and_constructor_errors`) in `crates/vm/src/lib.rs`.
- **Files modified:** `crates/vm/src/lib.rs`
- **Verification:** Both exact-name VM verify commands execute one passing test each.
- **Committed in:** `a3c642d`, `4376142`

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Verification reliability improved with no scope drift beyond the required command contract.

## Issues Encountered
None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- BUI-05 is observably closed with deterministic constructor/prototype behavior and regression coverage.
- Phase 6 is ready for Plan 03 CI/baseline gate wiring.

---
*Phase: 06-collection-and-regexp-semantics*
*Completed: 2026-02-27*
