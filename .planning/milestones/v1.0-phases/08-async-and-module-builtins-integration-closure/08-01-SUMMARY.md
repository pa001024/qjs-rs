---
phase: 08-async-and-module-builtins-integration-closure
plan: 01
subsystem: runtime
tags: [module, promise, builtins, harness]
requires:
  - phase: 07-compatibility-and-governance-gates
    provides: deterministic CI/test governance baseline
provides:
  - module realm now installs baseline builtins before module chunk execution
  - exact-name VM and harness regressions for Promise builtin parity on module path
  - harness module lifecycle assertions cover Promise parity, cache reuse, and deterministic failure replay
affects: [08-02-PLAN.md, 08-03-PLAN.md, verification-traceability]
tech-stack:
  added: []
  patterns:
    - module realm baseline wiring via builtins::install_baseline
key-files:
  created: []
  modified:
    - crates/vm/Cargo.toml
    - crates/vm/src/lib.rs
    - crates/vm/tests/module_lifecycle.rs
    - crates/test-harness/src/lib.rs
    - crates/test-harness/tests/module_lifecycle.rs
key-decisions:
  - Install baseline globals in module realm initialization via builtins::install_baseline before module chunk execution.
  - Keep ModuleLifecycle typed error mapping unchanged and only close missing-builtin behavior.
  - Use constructor-based Promise + then chain in parity tests to validate supported async surface deterministically.
patterns-established:
  - Exact-name module parity tests must exercise both direct Promise access and basic then chain creation.
  - Harness module lifecycle regressions should assert Promise availability together with cache reuse and deterministic replay.
requirements-completed: [ASY-01, ASY-02]
duration: 4 min
completed: 2026-02-27
---

# Phase 08 Plan 01: Reproduce and fix module realm baseline builtin availability gaps Summary

**Module execution now installs baseline globals so Promise-dependent module evaluation succeeds while lifecycle error tokens remain deterministic.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-27T10:04:46Z
- **Completed:** 2026-02-27T10:09:04Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Added deterministic VM/harness exact-name regressions that reproduce module Promise baseline gap.
- Wired module realm initialization to install baseline globals before executing compiled module chunks.
- Extended harness module lifecycle coverage to lock Promise parity, cache reuse, and deterministic EvaluateFailed replay.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add failing module-path Promise baseline reproductions with exact-name tests** - `34c1b2e` (test)
2. **Task 2: Install baseline builtins in module evaluation realm without weakening lifecycle error typing** - `428311e` (fix)
3. **Task 3: Lock harness-level module baseline parity through run_module_entry** - `4af7dbe` (test)

**Plan metadata:** `(pending)`

## Files Created/Modified
- `crates/vm/Cargo.toml` - Added `builtins` workspace dependency for module realm baseline wiring.
- `crates/vm/src/lib.rs` - Installed baseline globals in `execute_module_record` module realm setup.
- `crates/vm/tests/module_lifecycle.rs` - Added exact-name `module_promise_builtin_parity` regression and string-export helper.
- `crates/test-harness/src/lib.rs` - Documented that `run_module_entry` intentionally routes through `Vm::evaluate_module_entry`.
- `crates/test-harness/tests/module_lifecycle.rs` - Added exact-name module entry Promise parity test and extended cache/replay assertions.

## Decisions Made
- Centralized module-path baseline builtin setup at VM module realm initialization to match script-path contract.
- Preserved existing lifecycle failure typing (`ParseFailed`, `EvaluateFailed`, host-contract/load/resolve errors) with no token drift.
- Locked harness parity expectations through `run_module_entry` so integration tests exercise the same VM module lifecycle path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Promise parity tests initially used unsupported `Promise.resolve` static API**
- **Found during:** Task 2 (module builtin wiring verification)
- **Issue:** The new parity tests failed after builtin wiring because current runtime Promise surface does not implement `Promise.resolve`.
- **Fix:** Rewrote parity tests to use supported `new Promise(...).then(...)` chain while preserving direct Promise + then-chain coverage intent.
- **Files modified:** `crates/vm/tests/module_lifecycle.rs`, `crates/test-harness/tests/module_lifecycle.rs`
- **Verification:** `cargo test -p vm module_promise_builtin_parity -- --exact` and harness suite passed with deterministic assertions.
- **Committed in:** `428311e` and `4af7dbe` (task commits)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** No scope creep; deviation tightened tests to currently supported Promise semantics while preserving required parity coverage.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Module path Promise baseline gap is closed with deterministic VM+harness evidence.
- Ready for `08-02-PLAN.md` to validate Promise queue semantics and host-hook parity through module execution paths.

---
*Phase: 08-async-and-module-builtins-integration-closure*
*Completed: 2026-02-27*
