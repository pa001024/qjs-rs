# Phase 1: Semantic Core Closure - Context

**Gathered:** 2026-02-25
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase closes semantic gaps for `eval`, lexical scoping/closure behavior, control-flow completion values, and descriptor invariants.  
Scope is fixed to semantic correctness and deterministic behavior for capabilities already defined in Phase 1.

</domain>

<decisions>
## Implementation Decisions

### Eval behavior boundaries
- Implement direct vs indirect `eval` per target specification semantics; no project-specific simplification.
- Preserve strict vs non-strict `eval` behavior as defined by target semantics.
- Keep `indirect eval` visibility global-only; do not expose caller lexical bindings.
- Preserve original observable error categories and precedence (`SyntaxError`, `ReferenceError`, `TypeError`, etc.) rather than collapsing error types.
- Preserve target `this` behavior distinctions for direct/indirect and strict/non-strict execution.

### Lexical scope and closure capture
- Implement lexical environment rules strictly by target semantics across nested blocks, functions, and loops.
- Preserve shadowing and capture timing behavior per specification; no convenience rewrites.
- Keep behavior deterministic when lexical bindings interact with existing runtime constructs (including currently supported `with`/eval paths) without introducing custom semantics.

### Descriptor invariants
- Implement `Object.defineProperty`, `Object.defineProperties`, and `Object.getOwnPropertyDescriptor` edge behavior according to target descriptor invariants.
- Illegal descriptor transitions must fail deterministically with appropriate semantic error behavior (no silent fallback).
- Keep property attribute consistency (`configurable`, `enumerable`, `writable`, accessor/data exclusivity) strict and spec-aligned.

### Claude's Discretion
- Test organization, fixture naming, and coverage layering (unit/integration/test262 subset split) are at Claude's discretion.
- Internal refactor granularity is at Claude's discretion as long as semantics stay aligned and scope does not expand.

</decisions>

<specifics>
## Specific Ideas

- User direction: for selected gray areas in this phase, use specification-aligned behavior directly.
- User direction: avoid simplification policies that trade away semantic fidelity.
- User direction: no additional preference questions are needed when the choice is "follow spec".

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within phase scope.

</deferred>

---

*Phase: 01-semantic-core-closure*
*Context gathered: 2026-02-25*
