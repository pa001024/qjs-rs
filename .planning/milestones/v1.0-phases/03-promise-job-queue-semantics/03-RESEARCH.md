# Phase 3: Promise Job Queue Semantics - Research

**Researched:** 2026-02-26  
**Domain:** ASY-01 / ASY-02 Promise microtask semantics and host queue contract  
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Microtask Ordering Semantics
- Execute microtasks after the current synchronous turn completes; no eager per-settlement partial flush.
- Maintain strict FIFO queue order; nested Promise reactions append to queue tail.
- Route all `then/catch/finally` reactions through the unified Promise Job Queue (no synchronous fast path).
- Keep `finally` behavior spec-aligned (transparent pass-through unless `finally` itself throws/rejects).

#### Host Callback Contract Boundaries
- Keep host enqueue/drain behavior aligned to specification-facing semantics.
- Lock callback contract to deterministic queue state transitions and reproducible ordering.
- Reject or fail deterministically for invalid host-callback interaction paths; no silent fallback behavior.

#### Exception Propagation and Error Stability
- Promise handler exceptions propagate through the same queue semantics deterministically.
- Error behavior (type/category and observable propagation path) remains stable and regression-testable.
- No custom project-specific error shortcuts that diverge from the selected spec-aligned behavior.

### Claude's Discretion
- Internal type shapes and private helper boundaries for queue records/callback plumbing are at Claude's discretion.
- Test decomposition strategy (unit vs integration split and fixture naming) is at Claude's discretion as long as semantic contracts above remain locked.

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Requirement Mapping

| ID | Requirement | Implementation Target | Verification Target |
|----|-------------|-----------------------|---------------------|
| ASY-01 | Deterministic Promise settlement + microtask ordering for `then/catch/finally`. | Add single FIFO `PromiseJobQueue` in `Vm`; route all reactions to enqueue-only path; no sync execution path. | Ordering matrix tests (already-settled, nested enqueue, mixed fulfill/reject, `finally` pass-through/override). |
| ASY-02 | Safe host callbacks to enqueue and drain Promise jobs. | Add host-facing queue API and optional callbacks with strict reentrancy/state checks. | Host contract tests for enqueue, bounded drain, reentrant drain rejection, and deterministic error typing. |
</phase_requirements>

## Current Baseline (Repo Reality)

- Promise constructor/prototype exist, but `Promise.prototype.then/catch/finally` semantics are not implemented (`crates/vm/src/lib.rs:9264`, `crates/vm/src/lib.rs:15015`).
- Existing async tests only verify “returns Promise instance”, not microtask order or propagation (`crates/vm/src/lib.rs:22326`).
- Phase 2 root safety primitives already exist for pending jobs and can be reused (`crates/vm/src/lib.rs:738`, `crates/vm/src/lib.rs:934`, `crates/vm/src/lib.rs:22896`).

## Deterministic Microtask Queue Semantics

Use one VM-owned queue (`VecDeque<PromiseJob>`) with these fixed rules:

1. `enqueue(job)` always appends to tail.
2. Reactions from `then/catch/finally` are always queued, including already-settled promises.
3. `drain()` runs only after host/turn boundary trigger; never from inside settlement logic.
4. Nested jobs created while draining are appended and run later in the same drain loop (FIFO preserved).
5. Reentrant drain is deterministic: reject with typed error (preferred) or no-op with explicit status, but choose one and freeze it.
6. `finally` is transparent unless it throws/rejects.

## Host Enqueue/Drain Callback Contract Boundaries

Host boundary should be narrow and invariant-preserving:

- Allowed host operations:
  - `has_pending_promise_jobs() -> bool`
  - `drain_promise_jobs(budget) -> Result<DrainReport, VmError>`
  - Optional callbacks: `on_enqueue`, `on_drain_start`, `on_drain_end`
- Forbidden host operations:
  - Direct queue mutation
  - Drain recursion while `draining == true`
  - Silent fallback on invalid state transitions
- Contract guarantees:
  - Queue order and job count transitions are deterministic
  - Drain budget is honored exactly
  - Error category/message shape for invalid host interaction is stable for tests

## Exception Propagation Behavior

Promise job execution must preserve deterministic propagation:

1. `onFulfilled` throws -> returned promise is rejected with thrown value.
2. `onRejected` throws -> returned promise is rejected with thrown value.
3. `finally` callback returns normally -> original fulfillment/rejection passes through unchanged.
4. `finally` callback throws/rejects -> overrides prior state with new rejection.
5. Drain loop must continue after per-job promise rejections (normal Promise behavior), but stop on VM-fatal infrastructure errors with deterministic report.

## Pitfalls / Anti-Patterns

- Synchronous fast-path for settled promises.
- Push-front or recursive drain, which breaks FIFO.
- Queue internals exposed to host directly.
- Unstable or generic errors for host misuse (hard to regression test).
- Forgetting pending-job root candidate registration/release for captured values.

## Planning and Verification Gates

Gate 1: Queue Core Ready
- Build: `PromiseJob`, `PromiseJobQueue`, `enqueue`, `drain`, `draining` guard.
- Verify: unit tests for FIFO, nested append ordering, bounded drain.

Gate 2: Promise Chain Semantics Ready
- Build: typed promise state + reaction lists + `then/catch/finally` routing to queue.
- Verify: integration tests for ordering and `finally` transparency/override.

Gate 3: Host Contract Ready
- Build: host API for pending/drain + callback hooks + invalid-state checks.
- Verify: contract tests for non-reentrant drain, deterministic failure paths, stable drain report.

Gate 4: Exception and GC Stability Ready
- Build: deterministic propagation mapping and root registration for queued captures.
- Verify: mixed fulfill/reject/throw chain tests plus GC stress with queued jobs alive-until-run and reclaimed-after-release.

## Primary Recommendation

Implement Phase 3 in this order: queue core -> promise reaction wiring -> host contract -> exception/GC hardening, and gate each stage with deterministic behavior tests before moving forward.
