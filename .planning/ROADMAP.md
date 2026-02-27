# Roadmap: qjs-rs

## Overview

This roadmap closes remaining v1 semantic/runtime gaps first, then lands async and module behavior, then expands builtins, and finally hardens compatibility and governance signals. The sequence is dependency-driven so each phase delivers a coherent capability that unblocks the next.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

- [x] **Phase 1: Semantic Core Closure** - Close eval/scope/control-flow/descriptor semantic gaps in executable behavior. (completed 2026-02-25)
- [x] **Phase 2: Runtime Safety and Root Integrity** - Stabilize GC roots and stale-handle safety guarantees. (completed 2026-02-26)
- [x] **Phase 3: Promise Job Queue Semantics** - Deliver deterministic microtask behavior and host queue hooks. (completed 2026-02-26)
- [x] **Phase 4: ES Module Lifecycle** - Implement parse/instantiate/evaluate flow with deterministic cache and cycle handling. (completed 2026-02-26)
- [x] **Phase 5: Core Builtins Baseline** - Bring core constructors, error hierarchy, and JSON to target CI subsets. (completed 2026-02-26)
- [x] **Phase 6: Collection and RegExp Semantics** - Complete dedicated Map/Set and RegExp behavior without alias shortcuts. (completed 2026-02-27)
- [x] **Phase 7: Compatibility and Governance Gates** - Lock telemetry, reporting, and release-governance quality gates. (completed 2026-02-27)
- [ ] **Phase 8: Async and Module Builtins Integration Closure** - Close module-evaluation builtin wiring gaps and re-validate Promise queue semantics through module execution paths.
- [ ] **Phase 9: Verification Traceability Normalization** - Normalize verification schema and automation contracts so requirement coverage is machine-verifiable end-to-end.

## Phase Details

### Phase 1: Semantic Core Closure
**Goal**: Engine behavior for eval, lexical scoping, control-flow completion values, and descriptor invariants is deterministic and aligned to target semantics.
**Depends on**: Nothing (first phase)
**Requirements**: SEM-01, SEM-02, SEM-03, SEM-04
**Success Criteria** (what must be TRUE):
  1. Direct and indirect `eval` produce expected scope, strict-mode, and exception behavior in integration tests.
  2. Nested closures, block scopes, and function boundaries preserve lexical bindings under nested control flow.
  3. Completion values across `if/switch/label/try-finally/loop` paths are consistent and execute without panic paths.
  4. `Object.defineProperty/defineProperties/getOwnPropertyDescriptor` edge cases enforce descriptor invariants with deterministic failures for invalid transitions.
**Plans**: 3/3 plans complete

### Phase 2: Runtime Safety and Root Integrity
**Goal**: Runtime memory access is safe and deterministic under collection and handle lifecycle changes.
**Depends on**: Phase 1
**Requirements**: MEM-01, MEM-02
**Success Criteria** (what must be TRUE):
  1. GC root scanning includes stack frames, globals, module-cache candidates, and pending job queue references.
  2. Invalid or stale object handles are rejected with deterministic typed runtime errors.
  3. Stress scenarios with repeated allocation/collection do not produce stale-handle panics or undefined behavior.
**Plans**: 3/3 plans complete

### Phase 3: Promise Job Queue Semantics
**Goal**: Promise settlement and microtask execution are deterministic and safely controllable by embedding hosts.
**Depends on**: Phase 2
**Requirements**: ASY-01, ASY-02
**Success Criteria** (what must be TRUE):
  1. `then/catch/finally` chains execute in deterministic microtask order across nested resolution/rejection paths.
  2. Embedding code can enqueue and drain Promise jobs through runtime host callbacks without violating runtime safety.
  3. Promise handler exceptions propagate through the queue with reproducible error behavior.
**Plans**: 3/3 plans complete

### Phase 4: ES Module Lifecycle
**Goal**: ES module execution supports parse, instantiate, and evaluate with stable caching and cycle behavior.
**Depends on**: Phase 3
**Requirements**: MOD-01, MOD-02
**Success Criteria** (what must be TRUE):
  1. Static import/export module graphs complete parse, instantiate, and evaluate in integration tests.
  2. Repeated imports reuse module cache entries without re-instantiating completed modules.
  3. Cyclic module graphs execute in deterministic order with deterministic error propagation on failure.
**Plans**: 3/3 plans complete

### Phase 5: Core Builtins Baseline
**Goal**: Core builtin objects, error hierarchy, and JSON interop satisfy targeted conformance scenarios.
**Depends on**: Phase 4
**Requirements**: BUI-01, BUI-02, BUI-03
**Success Criteria** (what must be TRUE):
  1. `Object`, `Function`, `Array`, `String`, `Number`, `Boolean`, `Math`, and `Date` pass targeted CI conformance subsets.
  2. `Error` and standard subclasses expose expected constructor/prototype behavior and stringification.
  3. `JSON.parse` and `JSON.stringify` interoperate on baseline nested data and reject malformed input deterministically.
**Plans**: 3 plans
Plans:
- [x] 05-01-PLAN.md - Close Error hierarchy constructor/prototype/toString determinism and native-error subset gates.
- [x] 05-02-PLAN.md - Replace placeholder JSON parse/stringify with deterministic reviver/replacer/space/cycle semantics.
- [x] 05-03-PLAN.md - Close core builtin conformance clusters and lock Phase 5 CI subset contract.

### Phase 6: Collection and RegExp Semantics
**Goal**: Collections and regular expressions use dedicated semantics aligned with targeted runtime behavior.
**Depends on**: Phase 5
**Requirements**: BUI-04, BUI-05
**Success Criteria** (what must be TRUE):
  1. `Map/Set/WeakMap/WeakSet` constructors and methods use dedicated internal semantics rather than alias shortcuts.
  2. `RegExp` constructor and `exec/test/toString` preserve flags and supported-pattern match behavior.
  3. Collection and RegExp regression suites pass in CI alongside prior builtin coverage.
**Plans**: 3/3 plans complete
Plans:
- [x] 06-01-PLAN.md - De-alias weak collections and lock Map/Set/WeakMap/WeakSet dedicated runtime semantics.
- [x] 06-02-PLAN.md - Close RegExp constructor/exec/test/toString semantics with shared lastIndex behavior and deterministic errors.
- [x] 06-03-PLAN.md - Wire Phase 6 collection/RegExp regression gates into test262-lite, CI, and baseline docs.

### Phase 7: Compatibility and Governance Gates
**Goal**: Compatibility reporting and quality governance are repeatable, measurable, and enforceable.
**Depends on**: Phase 6
**Requirements**: MEM-03, TST-01, TST-02, TST-03, TST-04
**Success Criteria** (what must be TRUE):
  1. Default-branch CI remains green for `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`.
  2. GC telemetry emits baseline and stress profiles with documented thresholds and regression checks.
  3. test262 reports include discovered/executed/failed counts and explicit skip categories for each tracked run.
  4. New runtime features merged in this phase include at least one positive test and one boundary/error test.
  5. Compatibility snapshots are reproducible and `docs/current-status.md` is updated after major convergence work.
**Plans**: 3/3 plans complete
Plans:
- [x] 07-01-PLAN.md - Enforce hard-blocking CI governance gates and PR `1 + 1` runtime test compliance checks.
- [x] 07-02-PLAN.md - Upgrade test262 reporting to fixed JSON/Markdown schema with explicit skip categories.
- [x] 07-03-PLAN.md - Establish reproducible baseline/stress snapshot governance and manifest-backed status updates.

### Phase 8: Async and Module Builtins Integration Closure
**Goal**: Module execution paths preserve baseline builtin availability and Promise behavior so async/module flows are end-to-end deterministic.
**Depends on**: Phase 7
**Requirements**: ASY-01, ASY-02
**Gap Closure**: Closes v1.0 audit requirement gaps (`ASY-01`, `ASY-02`) plus reported module/builtins E2E break.
**Success Criteria** (what must be TRUE):
  1. Module evaluation path exposes required baseline globals (including Promise) with deterministic behavior parity to script path.
  2. Promise `then/catch/finally` ordering and host queue hooks are validated through module-executed scenarios, not only script-only scenarios.
  3. Regression tests reproduce and then close the reported `ModuleLifecycle:EvaluateFailed` Promise usage flow gap.
**Plans**: 3 plans
Plans:
- [ ] 08-01-PLAN.md - Reproduce and fix module realm baseline builtin availability gaps.
- [ ] 08-02-PLAN.md - Add module-path Promise queue regression matrix for ASY-01/ASY-02 semantics.
- [ ] 08-03-PLAN.md - Wire Phase 8 E2E module+async gates into harness/CI with deterministic evidence output.

### Phase 9: Verification Traceability Normalization
**Goal**: Verification artifacts and tooling contracts are schema-consistent so requirement coverage auditing is automated and reproducible.
**Depends on**: Phase 8
**Requirements**: None (audit integration debt closure)
**Gap Closure**: Closes v1.0 audit integration gaps on verification schema alignment and manual fallback paths.
**Success Criteria** (what must be TRUE):
  1. All phase verification files expose consistent machine-parseable requirement mapping fields.
  2. Requirement traceability checks no longer rely on manual fallback for frontmatter/schema mismatches.
  3. Milestone audit rerun can compute requirement coverage directly from standardized verification artifacts.
**Plans**: 2 plans
Plans:
- [ ] 09-01-PLAN.md - Standardize verification report schema and update legacy phase artifacts.
- [ ] 09-02-PLAN.md - Align verification tooling parsers and add schema conformance checks in CI.

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Semantic Core Closure | 3/3 | Complete    | 2026-02-25 |
| 2. Runtime Safety and Root Integrity | 3/3 | Complete    | 2026-02-26 |
| 3. Promise Job Queue Semantics | 3/3 | Complete    | 2026-02-26 |
| 4. ES Module Lifecycle | 3/3 | Complete    | 2026-02-26 |
| 5. Core Builtins Baseline | 3/3 | Complete    | 2026-02-26 |
| 6. Collection and RegExp Semantics | 3/3 | Complete    | 2026-02-27 |
| 7. Compatibility and Governance Gates | 3/3 | Complete    | 2026-02-27 |
| 8. Async and Module Builtins Integration Closure | 0/3 | Not started | - |
| 9. Verification Traceability Normalization | 0/2 | Not started | - |
