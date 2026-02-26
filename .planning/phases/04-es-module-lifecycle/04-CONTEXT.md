# Phase 4: ES Module Lifecycle - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers deterministic ES module lifecycle behavior for static import/export graphs: parse, instantiate, evaluate, cache reuse, and cyclic dependency handling with reproducible error propagation.

Out of scope for this phase: introducing new async capabilities, dynamic import feature expansion, or non-ESM host/runtime product surfaces.

</domain>

<decisions>
## Implementation Decisions

### Module Cache Identity and Reuse
- Cache identity is the resolved module specifier (post-resolution canonical key), not raw source text.
- A successfully instantiated/evaluated module must be reused on repeated imports without re-running side effects.
- Failed module records should preserve deterministic failure status for subsequent import attempts.

### Parse -> Instantiate -> Evaluate State Model
- Module records use explicit lifecycle states (`unlinked`, `linking`, `linked`, `evaluating`, `evaluated`, `errored`) to prevent ambiguous transitions.
- Instantiation resolves dependency graph first, then evaluation executes in deterministic order.
- State transitions are single-path and idempotent per module record.

### Cyclic Dependency Semantics
- Cycles are supported with deterministic traversal order and deterministic failure surface.
- Linking in cycles should avoid duplicate work and must not deadlock/recurse infinitely.
- If any module in a strongly connected segment fails during evaluate, downstream importers observe stable propagated failure semantics.

### Host Resolver / Loader Boundary
- Host integration surface stays minimal and deterministic: resolve, load source, and optional normalization hooks.
- VM/runtime retains ownership of lifecycle transitions and cache mutation; host does not mutate internal module graph state directly.
- Resolution/load failures map to stable typed runtime error behavior suitable for regression tests.

### Error Determinism and Observability
- Module parse/link/evaluate failures are categorized into deterministic error classes/messages.
- Re-import after failure should reproduce stable observable behavior (same failure category, same cache policy).
- Error paths must be regression-tested at integration level, not only unit level.

### Claude's Discretion
- Internal data structures for module records, dependency indexing, and graph traversal helpers.
- Exact helper API partition between `vm` and `runtime` while preserving the lifecycle contract above.
- Test matrix decomposition across VM unit tests and test-harness integration tests.

</decisions>

<specifics>
## Specific Ideas

- Prioritize behavior parity with QuickJS-style deterministic lifecycle semantics while keeping Rust-native maintainability.
- Keep deterministic ordering and reproducible failure surfaces as first-class acceptance criteria in tests and docs.

</specifics>

<deferred>
## Deferred Ideas

- Dynamic `import()` expansion and async module execution model refinements.
- Advanced host module formats beyond baseline static ESM graphs.

</deferred>

---

*Phase: 04-es-module-lifecycle*
*Context gathered: 2026-02-26*
