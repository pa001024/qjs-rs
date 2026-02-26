# Requirements: qjs-rs

**Defined:** 2026-02-25
**Core Value:** Deliver QuickJS-aligned JavaScript semantics in a pure Rust runtime without introducing C FFI into the runtime core.

## v1 Requirements

Requirements for the next roadmap cycle. Each maps to exactly one roadmap phase.

### Semantics Core

- [x] **SEM-01**: Engine executes direct and indirect `eval` with observable behavior aligned to current target semantics for scope, strict mode, and exception propagation.
- [x] **SEM-02**: Engine preserves lexical environment correctness for closures, block scopes, and function boundaries under nested control flow.
- [x] **SEM-03**: Engine implements consistent completion-value behavior across `if/switch/label/try-finally/loop` control flow without panic paths.
- [x] **SEM-04**: Engine enforces object/property descriptor invariants for `Object.defineProperty/defineProperties/getOwnPropertyDescriptor` in edge cases.

### Runtime and Memory

- [x] **MEM-01**: GC root management covers stack frames, globals, module cache candidates, and job queue references without stale-handle use.
- [ ] **MEM-02**: Runtime rejects invalid/stale object handles with deterministic typed errors instead of undefined behavior or panics.
- [ ] **MEM-03**: GC telemetry reports stable baseline and stress profiles with documented thresholds and regression checks.

### Async and Modules

- [ ] **ASY-01**: Promise settlement and microtask ordering follow deterministic queue semantics for `then/catch/finally` chains.
- [ ] **ASY-02**: Runtime exposes host callbacks to enqueue and drain Promise jobs safely from embedding code.
- [ ] **MOD-01**: ES module flow supports parse, instantiate, and evaluate for static import/export graphs.
- [ ] **MOD-02**: Module loader handles cache reuse and cyclic dependency execution order with deterministic error propagation.

### Builtins Coverage

- [ ] **BUI-01**: Core builtins (`Object`, `Function`, `Array`, `String`, `Number`, `Boolean`, `Math`, `Date`) satisfy targeted conformance subsets used in CI.
- [ ] **BUI-02**: Error hierarchy (`Error` plus standard subclasses) exposes expected constructor/prototype behavior and stringification.
- [ ] **BUI-03**: `JSON.parse` and `JSON.stringify` support baseline interoperability scenarios used by harness and integration tests.
- [ ] **BUI-04**: `Map/Set/WeakMap/WeakSet` use dedicated semantics (no baseline constructor alias shortcuts).
- [ ] **BUI-05**: RegExp constructor and prototype methods (`exec/test/toString`) preserve flags and match behavior for supported patterns.

### Conformance and Governance

- [ ] **TST-01**: CI keeps `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` green on default branch.
- [ ] **TST-02**: test262 reporting tracks discovered/executed/failed plus skip categories so coverage expansion is measurable.
- [ ] **TST-03**: Every new runtime feature lands with at least one positive test and one boundary/error test.
- [ ] **TST-04**: Project publishes repeatable compatibility snapshots and updates `docs/current-status.md` after major convergence work.

## v2 Requirements

Deferred to later cycles after v1 semantic closure.

### Language and Runtime Expansion

- **LAN-01**: Expand full `Proxy` invariant coverage beyond minimal currently executable paths.
- **LAN-02**: Expand `Symbol` and `BigInt` edge behavior to larger conformance subsets.
- **LAN-03**: Broaden typed-array coverage beyond baseline `Uint8Array`-centric paths.

### Performance and Productization

- **PERF-01**: Add stable benchmark suite comparing key scenarios against QuickJS and Boa.
- **PERF-02**: Introduce targeted optimizations after semantic/compatibility gates stabilize.
- **PROD-01**: Add optional CLI shell without changing library-first contract.

## Out of Scope

Explicit exclusions for this roadmap cycle.

| Feature | Reason |
|---------|--------|
| Runtime core C FFI dependency | Violates project boundary (pure Rust runtime core) |
| Node.js/Web API compatibility layer in core engine | Not required for current library-semantic milestones |
| JIT compiler or broad NaN-boxing redesign now | Premature before semantic and compatibility closure |
| Large host framework integration | Distracts from engine-core milestone completion |

## Traceability

Roadmap mapping table. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SEM-01 | Phase 1 | Complete |
| SEM-02 | Phase 1 | Complete |
| SEM-03 | Phase 1 | Complete |
| SEM-04 | Phase 1 | Complete |
| MEM-01 | Phase 2 | Complete |
| MEM-02 | Phase 2 | Pending |
| MEM-03 | Phase 7 | Pending |
| ASY-01 | Phase 3 | Pending |
| ASY-02 | Phase 3 | Pending |
| MOD-01 | Phase 4 | Pending |
| MOD-02 | Phase 4 | Pending |
| BUI-01 | Phase 5 | Pending |
| BUI-02 | Phase 5 | Pending |
| BUI-03 | Phase 5 | Pending |
| BUI-04 | Phase 6 | Pending |
| BUI-05 | Phase 6 | Pending |
| TST-01 | Phase 7 | Pending |
| TST-02 | Phase 7 | Pending |
| TST-03 | Phase 7 | Pending |
| TST-04 | Phase 7 | Pending |

**Coverage:**
- v1 requirements: 20 total
- Mapped to phases: 20
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-25*
*Last updated: 2026-02-26 after 02-01 recovery completion*
