# Phase 8: Async and Module Builtins Integration Closure - Research

**Researched:** 2026-02-27  
**Domain:** Module execution/builtins wiring + Promise queue parity across module paths (ASY-01, ASY-02)  
**Confidence:** HIGH

<user_constraints>
## User Constraints

- Must close Phase 8 requirement IDs: `ASY-01`, `ASY-02`.
- Must close reported E2E break: module evaluation can fail on Promise usage with `ModuleLifecycle:EvaluateFailed`.
- Must preserve deterministic behavior and existing quality gates from Phase 7.
- `CLAUDE.md` not present; `.agents/skills/` not present.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ASY-01 | Promise settlement and microtask ordering follow deterministic queue semantics for `then/catch/finally` chains. | Module execution currently runs in an isolated VM path, so ASY behavior is not validated through module flows; Phase 8 should add module-path queue tests and ensure queue visibility/behavior parity. |
| ASY-02 | Runtime exposes host callbacks to enqueue and drain Promise jobs safely from embedding code. | Host hooks exist on `Vm`, but module-evaluated Promise jobs are not currently proven to flow through the same observable host-hook surface; Phase 8 needs module-path host-hook evidence. |
</phase_requirements>

## Executive Summary

Phase 8 is mainly an integration closure, not new async feature invention. Promise queue semantics and host hooks were implemented in Phase 3, and module lifecycle was implemented in Phase 4, but the two systems are still weakly connected in runtime behavior and evidence.

The key planning fact: module execution currently creates a fresh `Realm::default()` and a fresh `Vm::default()` inside module evaluation, without baseline builtin install. That explains the reported Promise-in-module failure path (`ModuleLifecycle:EvaluateFailed`) and also means module-path Promise behavior is not naturally observable via the main VM host-hook queue APIs.

**Primary recommendation:** plan Phase 8 as a staged closure:
1. Reproduce + close baseline builtin gap in module path (08-01).
2. Close Promise queue/hook parity through module-executed scenarios with deterministic tests (08-02).
3. Wire deterministic CI/harness/verification evidence and requirement mapping so ASY-01/ASY-02 are no longer orphaned (08-03).

## Current State Diagnosis (Code-Level)

1. Script path installs baseline globals; module path does not.
- Script/harness path calls `install_baseline` before execution in `crates/test-harness/src/lib.rs`.
- Module path (`run_module_entry`) does not install baseline, and VM module execution builds a fresh `Realm::default()` in `crates/vm/src/lib.rs` (`execute_module_record`).

2. Module execution currently uses an isolated VM.
- `evaluate_module_entry` delegates to `execute_module_record`.
- `execute_module_record` constructs `let mut module_vm = Vm::default();` and executes module chunk there.
- This isolates Promise job queue state from the outer `Vm` instance where host hooks are usually exercised.

3. Existing module tests do not cover Promise/builtin parity through module execution.
- Current module suites (`crates/vm/tests/module_lifecycle.rs`, `crates/test-harness/tests/module_lifecycle.rs`) validate lifecycle/cache/cycle/failure replay, but not module-path Promise queue semantics.

4. test262-lite currently skips `flags: [module]` by design.
- `crates/test-harness/src/test262.rs` classifies module-flagged tests as skip category `flag_module`.
- Phase 8 should not depend on flipping full test262 module support; use dedicated deterministic integration tests for closure.

## Implementation Options

### Option A: Minimal Module Builtins Fix (Low Risk, Partial Closure)

**What changes**
- In module execution path, install baseline globals into module realm before executing compiled module chunk.
- Likely add `builtins` dependency to `vm` (or move baseline global setup into a shared runtime helper).

**Pros**
- Fastest path to close reported `Promise` missing failure.
- Minimal churn to module lifecycle state machine and cache behavior.

**Cons**
- Does not inherently prove ASY-01/ASY-02 parity through the *same* VM queue/hook surface because module code still runs in a separate VM instance.
- Can close the bug but may leave async integration debt.

**Fit**
- Good for 08-01 reproduction/fix.
- Insufficient as the only Phase 8 strategy.

### Option B: Shared-VM Module Execution Context (Higher Effort, Full Closure)

**What changes**
- Stop creating a separate `module_vm` in `execute_module_record`.
- Execute module chunks on the same `Vm` instance via inline execution context (likely around existing `execute_inline_chunk`) with temporary module scope/var-scope handling.
- Ensure module context has baseline globals + imported bindings without leaking module declarations into global script scope.

**Pros**
- Module-created Promise jobs become visible to the same `Vm` host-hook APIs (`drain_promise_jobs_with_host_hooks`), enabling direct ASY-01/ASY-02 module-path verification.
- Strongest semantic parity between script and module execution paths.

**Cons**
- Higher complexity: scope isolation, var-binding behavior, and avoiding `execute_in_realm` full reset semantics.
- Needs careful regression protection around module cache and GC root behavior.

**Fit**
- Best long-term closure for Phase 8 requirements.
- Should be implemented incrementally after 08-01 regression lock.

### Option C: Keep Isolated Module VM but Add Module-Scoped Hook API (Medium Risk, Medium Closure)

**What changes**
- Keep per-module internal VM execution model.
- Add module-evaluation APIs that explicitly expose module-local Promise drain/hook behavior to hosts.

**Pros**
- Lower refactor risk than Option B.

**Cons**
- Creates dual async surfaces (script VM hooks vs module-specific hooks).
- Increases API and maintenance complexity; weaker conceptual model.

**Fit**
- Acceptable fallback only if Option B scope becomes unsafe for the phase budget.

## Recommended Architecture Choice

Use **Option B as the target architecture**, but execute it in staged tasks:
- 08-01: lock failing reproduction + builtin parity (can temporarily use Option A mechanics).
- 08-02: complete shared-VM queue/hook parity through module path.
- 08-03: freeze evidence in CI/harness/verification artifacts.

This preserves momentum on the reported break while still delivering true ASY-01/ASY-02 closure quality.

## Risks and Mitigations

### Risk 1: Scope/global leakage when module code runs on shared VM
- **Why:** module declarations currently rely on isolated VM execution side effects.
- **Mitigation:** run module code in a temporary module scope stack (module var scope not global var scope), then restore prior VM scope state after module chunk execution.

### Risk 2: Reset semantics of `execute_in_realm` destroy module cache/queue context
- **Why:** `execute_in_realm` clears module cache, root candidates, and pending jobs by design.
- **Mitigation:** do not use full `execute_in_realm` inside module graph recursion; use inline execution helpers that preserve outer VM state.

### Risk 3: Regressions in deterministic error replay (`ModuleLifecycle:*`)
- **Why:** changing execution path can accidentally alter failure categorization.
- **Mitigation:** keep and extend existing exact-name module lifecycle replay tests; assert error tokens unchanged.

### Risk 4: False-green exact test commands
- **Why:** prior phases hit `--exact` name mismatches that executed zero tests.
- **Mitigation:** define top-level exact-name test functions matching CI commands.

### Risk 5: Overclaiming conformance due test262 module skips
- **Why:** module-flagged test262 cases are intentionally skipped.
- **Mitigation:** explicitly use dedicated module+async integration tests as phase evidence and document that full module-flag test262 enablement remains outside this phase.

## Acceptance Checks (Phase-Level)

Run these as Phase 8 gate set (names illustrative; planner should lock exact names):

```powershell
cargo test -p vm module_promise_builtin_parity -- --exact
cargo test -p vm module_promise_queue_semantics -- --exact
cargo test -p vm module_host_hook_drain_through_module_jobs -- --exact
cargo test -p vm module_error_replay_determinism -- --exact
cargo test -p vm module_cache_gc_root_integrity -- --exact
cargo test -p test-harness --test module_lifecycle
cargo test -p test-harness --test promise_job_queue
cargo test --workspace
```

If CI adds a dedicated Phase 8 block, include these exact commands there, similar to existing Phase 6 gates.

## Recommended Plan Decomposition

### 08-01: Reproduce and Fix Module Realm Builtin Availability Gap

**Intent**
- Make the reported Promise-in-module failure reproducible first, then close it with deterministic regression tests.

**Implementation focus**
- Add failing regression in VM/harness module lifecycle tests using module code that references `Promise` (and optionally one additional baseline builtin for parity confidence).
- Ensure module execution realm receives baseline globals (initially via minimal mechanism if needed).
- Keep `ModuleLifecycle:*` error mapping deterministic.

**Primary files likely touched**
- `crates/vm/src/lib.rs`
- `crates/vm/tests/module_lifecycle.rs`
- `crates/test-harness/tests/module_lifecycle.rs`
- optionally `crates/vm/Cargo.toml` (if adding `builtins` dep)

**Done criteria**
- Reproduction test fails before fix and passes after fix.
- Module evaluation with Promise no longer fails due missing builtin path.

### 08-02: Add Module-Path Promise Queue Regression Matrix (ASY-01/ASY-02)

**Intent**
- Prove Promise queue ordering and host-hook contract through module-executed flows, not script-only flows.

**Implementation focus**
- Ensure module-executed Promise jobs are observable via the same VM queue/hook APIs (preferred shared-VM module execution context).
- Add deterministic tests for:
  - FIFO ordering across module-triggered `then/catch/finally` chains
  - nested enqueue during drain
  - host hook callback ordering/counts (`on_enqueue`, `on_drain_start`, `on_drain_end`)
  - deterministic failures on invalid callback interaction
- Keep GC-root integrity checks for module-triggered queued captures.

**Primary files likely touched**
- `crates/vm/src/lib.rs`
- `crates/vm/tests/module_lifecycle.rs`
- `crates/test-harness/tests/promise_job_queue.rs`
- possibly new test file `crates/test-harness/tests/module_async_integration.rs`

**Done criteria**
- ASY-01 and ASY-02 have direct module-path evidence in tests, not inferred from script-path tests.

### 08-03: Wire E2E Gates and Verification Evidence

**Intent**
- Prevent regression and close audit traceability gaps with deterministic automation evidence.

**Implementation focus**
- Add/extend CI gate block for Phase 8 exact-name tests and integration tests.
- Update baseline docs with a Phase 8 command contract (similar format to Phase 5/6 entries).
- Produce a Phase 8 verification report that explicitly maps `ASY-01` and `ASY-02` evidence so these IDs are no longer orphaned in milestone audits.

**Primary files likely touched**
- `.github/workflows/ci.yml`
- `docs/test262-baseline.md`
- `.planning/phases/08-async-and-module-builtins-integration-closure/08-VERIFICATION.md` (later verification step)

**Done criteria**
- CI includes deterministic Phase 8 command chain.
- Documentation and verification artifacts explicitly bind Phase 8 evidence to `ASY-01`/`ASY-02`.

## Open Questions to Resolve During Planning

1. Should Phase 8 commit to full shared-VM module execution now, or allow a temporary minimal patch in 08-01 before completing parity in 08-02?
2. Do we keep current export-value sanitization (`non-primitive -> undefined`) unchanged in Phase 8, or treat it as out-of-scope technical debt?
3. Should module evaluation auto-drain Promise jobs, or keep host-driven drain as the only contract (recommended: host-driven only, for deterministic embedding control)?

## Sources

Primary code/docs reviewed:
- `.planning/STATE.md`
- `.planning/REQUIREMENTS.md`
- `.planning/ROADMAP.md`
- `.planning/v1.0-MILESTONE-AUDIT.md`
- `.planning/phases/03-promise-job-queue-semantics/03-VERIFICATION.md`
- `.planning/phases/04-es-module-lifecycle/04-VERIFICATION.md`
- `.planning/phases/07-compatibility-and-governance-gates/07-VERIFICATION.md`
- `crates/vm/src/lib.rs`
- `crates/vm/tests/module_lifecycle.rs`
- `crates/test-harness/src/lib.rs`
- `crates/test-harness/tests/module_lifecycle.rs`
- `crates/test-harness/tests/promise_job_queue.rs`
- `crates/test-harness/src/test262.rs`
- `.github/workflows/ci.yml`
- `docs/test262-baseline.md`

## Metadata

**Confidence breakdown**
- Root-cause diagnosis (module realm/builtin gap): HIGH
- Async integration gap diagnosis (module path vs host queue surface): HIGH
- Refactor complexity estimate for shared-VM module execution: MEDIUM-HIGH

**Research date:** 2026-02-27  
**Valid until:** 2026-03-13

## RESEARCH COMPLETE
