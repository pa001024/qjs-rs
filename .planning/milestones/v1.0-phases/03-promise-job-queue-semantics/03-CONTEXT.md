# Phase 3: Promise Job Queue Semantics - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase makes Promise settlement and microtask execution deterministic, and exposes safe host callbacks for enqueue/drain behavior.  
Scope is fixed to Phase 3 requirements (`ASY-01`, `ASY-02`) and does not expand into new async capability domains.

</domain>

<decisions>
## Implementation Decisions

### Microtask Ordering Semantics
- Execute microtasks after the current synchronous turn completes; no eager per-settlement partial flush.
- Maintain strict FIFO queue order; nested Promise reactions append to queue tail.
- Route all `then/catch/finally` reactions through the unified Promise Job Queue (no synchronous fast path).
- Keep `finally` behavior spec-aligned (transparent pass-through unless `finally` itself throws/rejects).

### Host Callback Contract Boundaries
- Keep host enqueue/drain behavior aligned to specification-facing semantics.
- Lock callback contract to deterministic queue state transitions and reproducible ordering.
- Reject or fail deterministically for invalid host-callback interaction paths; no silent fallback behavior.

### Exception Propagation and Error Stability
- Promise handler exceptions propagate through the same queue semantics deterministically.
- Error behavior (type/category and observable propagation path) remains stable and regression-testable.
- No custom project-specific error shortcuts that diverge from the selected spec-aligned behavior.

### Claude's Discretion
- Internal type shapes and private helper boundaries for queue records/callback plumbing are at Claude's discretion.
- Test decomposition strategy (unit vs integration split and fixture naming) is at Claude's discretion as long as semantic contracts above remain locked.

</decisions>

<specifics>
## Specific Ideas

- User direction: selected discussion areas (ordering, host contract, exception behavior) all use strict spec-aligned semantics.
- User direction: no additional policy simplifications are desired for Phase 3.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 03-promise-job-queue-semantics*
*Context gathered: 2026-02-26*
