---
phase: 05-core-builtins-baseline
plan: 02
subsystem: runtime
tags: [json, reviver, replacer, test262-lite, syntaxerror, typeerror]

requires:
  - phase: 05-core-builtins-baseline
    provides: native-error prototype and typed-error constructor determinism
provides:
  - deterministic JSON.parse baseline with reviver traversal and SyntaxError-category malformed-input failures
  - deterministic JSON.stringify baseline with replacer/space handling and cycle TypeError failures
  - dedicated json_interop harness regressions and built-ins/JSON test262-lite subset gate
affects: [phase-05-core-builtins-baseline, phase-07-compatibility-and-governance-gates]

tech-stack:
  added: [serde_json]
  patterns: [json-reviver-postwalk, json-stringify-cycle-guard, builtins-json-smoke-fixtures]

key-files:
  created:
    - .planning/phases/05-core-builtins-baseline/05-02-SUMMARY.md
    - crates/vm/tests/json_parse.rs
    - crates/vm/tests/json_stringify.rs
    - crates/test-harness/tests/json_interop.rs
    - crates/test-harness/fixtures/test262-lite/pass/built-ins/JSON/parse-reviver-smoke.js
    - crates/test-harness/fixtures/test262-lite/pass/built-ins/JSON/stringify-replacer-space-smoke.js
    - crates/test-harness/fixtures/test262-lite/pass/built-ins/JSON/stringify-cycle-smoke.js
  modified:
    - crates/vm/Cargo.toml
    - crates/vm/src/lib.rs
    - crates/test-harness/tests/test262_lite.rs

key-decisions:
  - "Use serde_json for baseline JSON grammar decoding, then convert into VM values before applying reviver semantics."
  - "Implement stringify traversal inside VM with explicit cycle stack and deterministic pretty-print assembly instead of placeholder object collapse."
  - "Lock BUI-03 with both direct json_interop runtime tests and local built-ins/JSON test262-lite smoke fixtures."

patterns-established:
  - "Exact-name VM verification tests: top-level integration test names match --exact verify commands for plan gates."
  - "JSON conformance gate pattern: runtime regressions in test-harness plus built-ins/JSON fixture subset in test262_lite.rs."

requirements-completed: [BUI-03]
duration: 10 min
completed: 2026-02-26
---

# Phase 5 Plan 02: JSON Interop Determinism Summary

**JSON.parse/JSON.stringify now run deterministic baseline algorithms with reviver/replacer/space semantics, typed malformed/cycle failures, and regression-locked harness/test262-lite coverage.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-02-26T17:35:03Z
- **Completed:** 2026-02-26T17:45:58Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments
- Replaced placeholder parse behavior with full baseline JSON value decoding and post-parse reviver walk semantics.
- Replaced placeholder stringify behavior with deterministic traversal for objects/arrays, replacer function/array handling, `space` clamping, unsupported-value filtering, and cycle TypeError errors.
- Added direct interoperability regressions plus a built-ins/JSON test262-lite subset gate to keep BUI-03 stable in CI-oriented runs.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement baseline JSON.parse with deterministic SyntaxError and reviver walk** - `14b910c` (feat)
2. **Task 2: Implement baseline JSON.stringify with replacer/space and cycle TypeError** - `93b7cda` (feat)
3. **Task 3: Add JSON interoperability regression suite and lite conformance subset gate** - `99707d8` (test)

**Plan metadata:** pending

## Files Created/Modified
- `crates/vm/src/lib.rs` - Implemented JSON parse/stringify algorithms with reviver/replacer/space/cycle behavior.
- `crates/vm/Cargo.toml` - Added `serde_json` dependency for deterministic JSON grammar decoding.
- `crates/vm/tests/json_parse.rs` - Added exact-name parse verification test.
- `crates/vm/tests/json_stringify.rs` - Added exact-name stringify verification test.
- `crates/test-harness/tests/json_interop.rs` - Added runtime-level JSON interoperability regressions.
- `crates/test-harness/tests/test262_lite.rs` - Added `json_subset` smoke gate.
- `crates/test-harness/fixtures/test262-lite/pass/built-ins/JSON/parse-reviver-smoke.js` - Added parse + reviver + malformed input fixture.
- `crates/test-harness/fixtures/test262-lite/pass/built-ins/JSON/stringify-replacer-space-smoke.js` - Added stringify replacer/space fixture.
- `crates/test-harness/fixtures/test262-lite/pass/built-ins/JSON/stringify-cycle-smoke.js` - Added cycle TypeError fixture.

## Decisions Made
- Kept malformed parse diagnostics deterministic by mapping decode failures to a stable SyntaxError category message.
- Used object-marker based array detection in stringify/reviver traversal to align with current VM array semantics and keep output ordering predictable.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- BUI-03 is now closed and regression-gated for both VM exact tests and harness/test262-lite smoke coverage.
- Ready for `05-03-PLAN.md` to close remaining Phase 5 core builtin subset clusters.

---
*Phase: 05-core-builtins-baseline*
*Completed: 2026-02-26*
