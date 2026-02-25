# Phase 2: Runtime Safety and Root Integrity - Context

**Gathered:** 2026-02-25
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase ensures runtime memory access is safe and deterministic under GC/root scanning and handle lifecycle transitions.  
Scope is fixed to root coverage integrity and stale/invalid handle safety guarantees required by Phase 2 (`MEM-01`, `MEM-02`).

</domain>

<decisions>
## Implementation Decisions

### Invalid/Stale Handle Error Contract
- Use deterministic typed runtime errors for all invalid/stale handle accesses; no silent recovery behavior.
- Keep error categories distinct for `InvalidHandle` vs `StaleHandle` to preserve diagnosis quality.
- Keep error type and message format stable enough for regression assertions.
- Fail fast at the first deterministic detection point.

### Root Coverage Boundary (Phase 2)
- Phase 2 must cover root scanning for stack frames, globals, module-cache candidates, and pending job-queue references.
- Do not defer any of these root categories to later phases.
- Root completeness in this phase is a hard gate, not a best-effort target.

### Safety Gate Pass Criteria
- Phase exit requires both functional correctness and stress-profile stability evidence.
- “No panic/undefined behavior under repeated allocation + collection” is mandatory.
- Deterministic typed failures are required for invalid-state paths.

### Exceptional Path Consistency
- Exceptional paths (GC checkpoints, stale-handle accesses, boundary failures) must be as deterministic and diagnosable as normal paths.
- Error contracts and observability are consistent across normal and exceptional control paths.

### Claude's Discretion
- Internal refactor layout and test-file decomposition are at Claude’s discretion.
- Specific naming of helper APIs and private runtime structs is at Claude’s discretion, as long as external semantic behavior matches locked decisions.

</decisions>

<specifics>
## Specific Ideas

- User direction: all selected Phase 2 decision areas should follow the recommended strict-safety path.
- User direction: no simplification or partial-coverage compromise is desired for this phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 02-runtime-safety-and-root-integrity*
*Context gathered: 2026-02-25*
