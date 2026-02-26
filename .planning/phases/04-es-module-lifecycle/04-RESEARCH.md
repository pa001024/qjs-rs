# Phase 4: ES Module Lifecycle - Research

**Researched:** 2026-02-26  
**Domain:** MOD-01 / MOD-02 static ES module parse/instantiate/evaluate lifecycle  
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Module Cache Identity and Reuse
- Cache key is canonical resolved specifier.
- Re-import of completed modules reuses cache without re-evaluation side effects.
- Failed module records keep deterministic failed state for subsequent imports.

#### Lifecycle and Cycle Semantics
- Explicit lifecycle state model (`unlinked`, `linking`, `linked`, `evaluating`, `evaluated`, `errored`).
- Deterministic parse -> instantiate -> evaluate transitions.
- Cyclic graphs must execute deterministically and propagate deterministic failures.

#### Host Contract and Error Determinism
- Host boundary remains resolve/load oriented; no direct mutation of VM module graph internals.
- Parse/link/evaluate failures must have stable typed error behavior and reproducible re-import semantics.

### Claude's Discretion
- Internal record structures and traversal helpers.
- VM/runtime split for lifecycle helpers.
- Test matrix partitioning across VM and harness.

### Deferred Ideas (OUT OF SCOPE)
- Dynamic `import()` and async ESM execution model expansion.
- Additional host module format surfaces.
</user_constraints>

<phase_requirements>
## Requirement Mapping

| ID | Requirement | Implementation Target | Verification Target |
|----|-------------|-----------------------|---------------------|
| MOD-01 | Parse/instantiate/evaluate static ESM graphs. | Module entry APIs, module record graph, ordered instantiate/evaluate pipeline. | Integration tests for static graphs with exports/imports and side-effect ordering. |
| MOD-02 | Deterministic cache reuse + cycle behavior + deterministic errors. | Canonical-key module cache with stateful records and cycle-safe traversal. | Re-import/cycle/failure replay tests with stable error category/message shape. |
</phase_requirements>

## Current Baseline (Repo Reality)

- Parser has script-oriented entry points; module keywords exist lexically but no module lifecycle runtime path is wired (`crates/parser/src/lib.rs:1003`, `crates/parser/src/lib.rs:1010`).
- VM already has module-cache GC root candidate infrastructure from Phase 2 but no module graph execution path (`crates/vm/src/lib.rs:823`, `crates/vm/src/lib.rs:1126`).
- No existing harness tests for module parse/instantiate/evaluate semantics in `test-harness`.

## Design Implications

1. **Stateful module record is mandatory**
   - Need explicit status transitions and guardrails to prevent illegal reentry.
   - Cache entries should outlive one execution turn and be rooted safely.

2. **Two-pass graph flow**
   - Instantiate pass: resolve/import graph and bind exported interfaces.
   - Evaluate pass: execute in deterministic post-order or SCC-aware order.

3. **Cycle handling strategy**
   - Maintain visitation marks per phase (instantiate/evaluate).
   - Use deterministic traversal ordering over import list order.
   - Preserve single failure source and stable propagated error behavior.

4. **Host loader boundary**
   - Keep a narrow API for resolve/load that returns canonical key + source.
   - Enforce VM-owned state transitions regardless of host behavior.

## Risks and Controls

- **Risk:** Parser/bytecode module entrypoint additions introduce scope/hoist regressions in script path.
  - **Control:** Keep module compile path parallel to script path and lock with script regression subset in CI.

- **Risk:** Cycle algorithm accidentally re-evaluates modules.
  - **Control:** Status guards plus exact re-entry tests for evaluating/evaluated states.

- **Risk:** Cache failure replay behavior diverges over time.
  - **Control:** Freeze failure storage and add deterministic re-import tests for same module key.

## Primary Recommendation

Implement in three waves:
1. Module record/cache + host load contract + state guards.
2. Instantiate/evaluate pipeline with cycle-safe deterministic traversal.
3. End-to-end harness and VM tests for cache reuse, cycle ordering, and failure replay semantics.
