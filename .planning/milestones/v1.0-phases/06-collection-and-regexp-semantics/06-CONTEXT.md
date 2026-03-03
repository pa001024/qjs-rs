# Phase 6: Collection and RegExp Semantics - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers dedicated, deterministic runtime semantics for `Map`, `Set`, `WeakMap`, `WeakSet`, and `RegExp` (`constructor`, `exec`, `test`, `toString`) with CI-regression gates aligned to roadmap Phase 6 requirements.

Out of scope: new builtin families or capability expansion beyond collection/regexp semantics already defined in roadmap Phase 6.

</domain>

<decisions>
## Implementation Decisions

### Collection Semantics Baseline
- Key equality for collections uses `SameValueZero` semantics (`NaN` equal to `NaN`, `+0` and `-0` treated as the same key).
- Insertion order follows spec-aligned behavior: `set` on an existing key does not move position; `delete` then re-`set` appends to the end.
- Iteration uses live-view semantics under in-loop mutation, not snapshot semantics.
- Phase 6 acceptance for collections prioritizes a core closure: constructor and baseline methods (`get/set/add/has/delete/clear/size/forEach/iterator`) plus boundary/error assertions.

### WeakMap / WeakSet Constraints
- Non-object keys are rejected immediately with deterministic `TypeError`.
- Iterable-constructor input with an invalid entry fails fast (throw on first invalid element, stop processing).
- API surface should stay shape-aligned with `Map`/`Set` where possible, while preserving strict weak-collection constraints.
- Weak-collection acceptance emphasizes object-key restrictions, core methods, and deterministic error behavior.

### RegExp Behavioral Baseline
- Phase 6 baseline supports flags `g/i/m/s/u/y`.
- Unsupported patterns or unsupported flags must fail deterministically at construction with `SyntaxError`.
- `exec` and `test` must share one matching core and consistent `lastIndex` semantics.
- `RegExp.prototype.toString` should return stable normalized `/source/flags` output.

### CI Regression Contract
- Use a three-layer gate for this phase: VM unit/integration tests, harness integration tests, and test262-lite subset gates.
- Newly introduced Phase 6 subset gates are expected to be fully green.
- Existing Phase 5 gates must not regress while Phase 6 changes land.
- Document fixed CI command contracts and expected baseline outputs in phase baseline docs.

### Claude's Discretion
- Internal helper decomposition and code organization as long as observable behavior and locked acceptance gates remain unchanged.
- Exact naming/layout of fixtures and test groupings, provided command contract stability is preserved.
- Sequencing of implementation across collections vs regexp internals, as long as dependency correctness and non-regression constraints are maintained.

</decisions>

<specifics>
## Specific Ideas

- Use recommended defaults as locked decisions for all discussed gray areas.
- Keep phase acceptance deterministic-first: behavior truth + boundary/error coverage over API-surface counting.

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within Phase 6 scope.

</deferred>

---

*Phase: 06-collection-and-regexp-semantics*
*Context gathered: 2026-02-26*
