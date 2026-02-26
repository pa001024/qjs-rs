---
phase: 05-core-builtins-baseline
plan: 03
subsystem: runtime
tags: [core-builtins, function-constructor, number-static, math-surface, date-baseline, test262-lite]

requires:
  - phase: 05-core-builtins-baseline
    provides: deterministic native error and JSON baseline closures from plans 01/02
provides:
  - exact-name VM gates for Object/Array/Boolean/Function and String/Number/Math baseline clusters
  - deterministic Number static helpers and expanded Math callable surface for phase-5 subset usage
  - deterministic Date constructor/parse/UTC/getTime/toString/toUTCString baseline with UTC-focused formatting
  - phase-local test262-lite built-ins subset fixtures for Object/Array/Boolean/Function/String/Number/Math/Date
affects: [phase-05-core-builtins-baseline, phase-07-compatibility-and-governance-gates]

tech-stack:
  added: []
  patterns: [exact-name-vm-gates, phase5-builtins-smoke-fixtures, utc-stable-date-format]

key-files:
  created:
    - .planning/phases/05-core-builtins-baseline/05-03-SUMMARY.md
    - crates/vm/tests/core_builtins_baseline.rs
    - crates/test-harness/tests/core_builtins_baseline.rs
    - crates/test-harness/fixtures/test262-lite/pass/built-ins/Date/core-smoke.js
    - crates/test-harness/fixtures/test262-lite/pass/built-ins/Math/core-smoke.js
  modified:
    - crates/runtime/src/lib.rs
    - crates/vm/src/lib.rs
    - crates/test-harness/tests/test262_lite.rs
    - docs/test262-baseline.md

key-decisions:
  - "Function constructor argument/body coercion switched to runtime ToString to preserve throwable coercion semantics in baseline gates."
  - "Phase-5 Number/Math closure uses explicit static Number guards and widened Math callable surface rather than narrowing subset expectations."
  - "Date string outputs are normalized to UTC RFC1123-style text for deterministic CI behavior across locales."

patterns-established:
  - "Phase-5 core builtins contract pattern: exact VM tests plus test-harness integration gate plus family-scoped test262-lite smoke fixtures."

requirements-completed: [BUI-01]

# Metrics
duration: 9 min
completed: 2026-02-26
---

# Phase 5 Plan 03: Core Builtins Baseline Closure Summary

**Phase-5 core builtins now have deterministic regression gates across VM exact tests, harness integration tests, and family-scoped test262-lite smoke subsets with Date behavior normalized to UTC-stable outputs.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-26T18:03:08Z
- **Completed:** 2026-02-26T18:12:07Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments
- Locked Object/Array/Boolean non-regression and Function constructor/prototype baseline edges with an exact-name VM verification gate.
- Closed String/Number/Math deterministic gaps by wiring Number static predicates (`isFinite/isInteger/isSafeInteger`), runtime-coercing `String.fromCharCode`, and expanding missing Math callable methods.
- Closed Date deterministic baseline and wired Phase-5 CI subset contract via dedicated harness assertions, new test262-lite family fixtures, and updated baseline docs.

## Task Commits

Each task was committed atomically:

1. **Task 1: Freeze Object/Array/Boolean non-regression and close Function constructor edges** - `109addd` (feat)
2. **Task 2: Close String then Number/Math deterministic gaps in locked priority order** - `f1cd4e4` (feat)
3. **Task 3: Close Date deterministic baseline and wire Phase-5 subset CI contract** - `e85f562` (feat)

**Plan metadata:** pending

## Files Created/Modified
- `crates/vm/src/lib.rs` - Added deterministic builtin closures for Function constructor coercion, Number static methods, expanded Math surface, and Date parse/UTC/string behavior.
- `crates/runtime/src/lib.rs` - Added NativeFunction variants for Number static predicates and expanded Math method surface.
- `crates/vm/tests/core_builtins_baseline.rs` - Added exact-name VM gates required by plan verify commands.
- `crates/test-harness/tests/core_builtins_baseline.rs` - Added integration regressions for object/function, string/number/math, and date baseline behaviors.
- `crates/test-harness/tests/test262_lite.rs` - Added `core_builtins_subset` family gate.
- `crates/test-harness/fixtures/test262-lite/pass/built-ins/*/core-smoke.js` - Added family smoke fixtures for Object, Array, Boolean, Function, String, Number, Math, and Date.
- `docs/test262-baseline.md` - Documented Phase-5 core builtin command contract and current deterministic results.

## Decisions Made
- Kept Phase-5 Date string outputs UTC-stable (`Thu, 02 Jan 2020 03:04:05 GMT`) to avoid locale-fragile CI expectations while preserving deterministic parse/round-trip gates.
- Added method-surface closure on Math/Number directly in VM native dispatch to reduce NotCallable regressions in targeted subsets without broad architectural churn.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- BUI-01 is now closed with deterministic VM, harness, and lite-subset gates.
- Phase 5 plan set is complete and ready for phase transition workflow.

---
*Phase: 05-core-builtins-baseline*
*Completed: 2026-02-26*
