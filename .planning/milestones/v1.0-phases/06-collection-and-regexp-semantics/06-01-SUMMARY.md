---
phase: 06-collection-and-regexp-semantics
plan: 01
subsystem: runtime
tags: [collections, weakmap, weakset, map-set-semantics, regression-gates]

requires:
  - phase: 05-core-builtins-baseline
    provides: deterministic builtin constructor/prototype wiring and baseline test262-lite gates
provides:
  - dedicated `WeakMap` and `WeakSet` constructors/prototypes (no `Map`/`Set` aliasing)
  - strict weak-collection non-object key `TypeError` behavior in constructor and method paths
  - exact-name VM and harness integration regression gates for collection semantics
affects: [phase-06-collection-and-regexp-semantics, phase-07-compatibility-and-governance-gates]

tech-stack:
  added: []
  patterns: [dedicated-weak-constructors, collection-fail-fast-iterable-ingestion, exact-name-vm-gates]

key-files:
  created:
    - .planning/phases/06-collection-and-regexp-semantics/06-01-SUMMARY.md
    - crates/vm/tests/collection_semantics.rs
    - crates/test-harness/tests/collection_semantics.rs
  modified:
    - crates/runtime/src/lib.rs
    - crates/builtins/src/lib.rs
    - crates/vm/src/lib.rs

key-decisions:
  - "Split weak collections into dedicated NativeFunction variants and dedicated prototype builders instead of aliasing to Map/Set constructors."
  - "Treat non-object weak keys as deterministic TypeError in both constructor iterable ingestion and WeakMap/WeakSet method dispatch."
  - "Keep Map/Set SameValueZero and live-iteration semantics locked via exact-name VM tests plus script-level harness integration."

patterns-established:
  - "When plan verify commands use `--exact`, back them with dedicated `crates/vm/tests/*` integration tests named exactly to the command target."

requirements-completed: [BUI-04]

duration: 5 min
completed: 2026-02-27
---

# Phase 6 Plan 01: Weak Collection De-alias and Collection Semantics Summary

**Weak collections now use dedicated constructor/prototype/runtime dispatch paths while Map/Set SameValueZero and live-iteration semantics are regression-locked in VM and harness tests.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-27T04:14:29Z
- **Completed:** 2026-02-27T04:19:13Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Replaced `WeakMap`/`WeakSet` global alias wiring with dedicated native constructors and dedicated prototype objects.
- Enforced weak-key object validation for constructor ingestion and `WeakMap`/`WeakSet` method calls with deterministic `TypeError` failures.
- Added focused VM and harness collection regression suites for constructor identity, SameValueZero semantics, live mutation iteration, and weak fail-fast behavior.

## Task Commits

Each task was committed atomically:

1. **Task 1: Split WeakMap/WeakSet constructor identity from Map/Set alias paths** - `fd42b16` (feat)
2. **Task 2: Enforce weak-key constraints and lock Map/Set baseline semantics** - `0560931` (test)
3. **Task 3: Add harness integration coverage for collection baseline and error edges** - `34736d2` (test)

**Plan metadata:** pending

## Files Created/Modified
- `crates/runtime/src/lib.rs` - Added dedicated weak collection native constructor variants.
- `crates/builtins/src/lib.rs` - Registered `WeakMap` and `WeakSet` globals to dedicated native constructors.
- `crates/vm/src/lib.rs` - Added dedicated weak constructors/prototypes/host-method dispatch and weak-key validation in iterable+method paths.
- `crates/vm/tests/collection_semantics.rs` - Added exact-name VM tests for plan verify commands and weak iterable fail-fast pull guards.
- `crates/test-harness/tests/collection_semantics.rs` - Added script-level integration suite for constructor identity, map/set semantics, and weak error edges.

## Decisions Made
- Dedicated weak collection constructor/prototype identity was implemented in VM internals rather than preserving shape-compatible aliases.
- Weak key validation is enforced consistently (constructor + methods) with deterministic `TypeError` behavior.
- Exact command contract tests were added in VM integration tests to ensure `cargo test -p vm ... --exact` executes concrete gates.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Released locked test binary before harness verification**
- **Found during:** Task 3 verification
- **Issue:** `cargo test -p test-harness --test collection_semantics` failed with `os error 5` because `target/debug/test262-run.exe` was locked by stale processes.
- **Fix:** Terminated stale `cargo`/`test262-run` processes and reran verification.
- **Files modified:** None
- **Verification:** `cargo test -p test-harness --test collection_semantics` passed after process cleanup.
- **Committed in:** N/A (environment/process fix only)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** No scope change. Unblocked planned verification and preserved required command contract.

## Issues Encountered

- Temporary local file-lock contention on `target/debug/test262-run.exe` during harness test build; resolved by stopping stale processes and rerunning.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- BUI-04 is closed with dedicated weak collection semantics and regression gates.
- Phase 6 is ready for Plan 02 (`RegExp` constructor/exec/test/toString semantics closure).

---
*Phase: 06-collection-and-regexp-semantics*
*Completed: 2026-02-27*
