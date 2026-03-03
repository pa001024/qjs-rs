# Phase 5: Core Builtins Baseline - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers deterministic baseline behavior for core builtins (`Object`, `Function`, `Array`, `String`, `Number`, `Boolean`, `Math`, `Date`), the standard `Error` hierarchy, and `JSON` interoperability in targeted CI subsets.

Out of scope: new builtin families (`Map/Set/Weak*`, `RegExp`, `Symbol`, `BigInt`) and any capability expansion outside roadmap Phase 5.

</domain>

<decisions>
## Implementation Decisions

### Builtin Coverage Prioritization
- Use layered rollout: stabilize must-have semantics first, then expand secondary edges.
- `Object`/`Function` must reach core semantic closure in this phase.
- Priority order for implementation and stabilization: `Array -> String -> Number/Math -> Date -> Boolean`.
- Acceptance uses behavior-truth gates, not API-count gates: each builtin area requires positive and boundary/error assertions.

### Error Hierarchy External Behavior
- Phase 5 includes: `Error`, `TypeError`, `ReferenceError`, `SyntaxError`, `RangeError`, `EvalError`, `URIError`.
- `name`/`message` follow standard constructor defaults and observable override behavior.
- `Error.prototype.toString` must follow standard name/message combination semantics.
- `instanceof` checks are strict acceptance criteria: subclass instances must satisfy both subclass and `Error` chains.

### JSON Behavioral Surface
- `JSON.parse` scope includes baseline parse plus `reviver` semantics.
- `JSON.stringify` scope includes baseline stringify plus `replacer` and `space` semantics.
- Invalid input/cycle paths must produce deterministic `TypeError`/`SyntaxError` category behavior.
- Object serialization output ordering must remain stable and testable under current engine guarantees.

### Date/Math/Number Determinism Baseline
- `Date` baseline closure for this phase includes constructor behavior, `Date.parse`, `Date.UTC`, `getTime`, `toString`, and `toUTCString`.
- Date tests should bias to UTC/timestamp assertions; avoid over-constraining locale-dependent text outputs.
- `Math`/`Number` acceptance prioritizes edge consistency (`NaN`, `Infinity`, `-0`, rounding/format paths).
- `Boolean`/`Number` boxing must be explicitly differentiated (`call` vs `new`) with observable `valueOf`/`toString` behavior.

### Claude's Discretion
- Internal task partitioning and exact sequencing inside each prioritized builtin block.
- Exact fixture shape and assertion decomposition, as long as the locked acceptance gates above are preserved.
- Internal helper/API refactor strategy used to reduce duplication while preserving current external behavior.

</decisions>

<specifics>
## Specific Ideas

- Use the recommended default choices for all discussed gray areas as hard planning input.
- Keep tests deterministic-first and regression-friendly instead of maximizing surface area per commit.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within Phase 5 scope.

</deferred>

---

*Phase: 05-core-builtins-baseline*
*Context gathered: 2026-02-26*
