# Feature Research

**Domain:** Pure Rust embeddable JavaScript runtime engine (QuickJS-aligned semantics)
**Researched:** 2026-02-25
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| ECMAScript core execution correctness (scope, strict mode, closures, exceptions, eval/with edges) | Engine credibility is judged first by language-correct behavior, not API surface | HIGH | Depends on parser/bytecode/vm/runtime semantic alignment; still active in local status docs |
| Object model + prototype chain + property descriptor invariants | Most built-ins and user code behavior depend on `[[Get]]/[[Set]]`, descriptors, and inheritance rules | HIGH | Prerequisite for reliable `Object.*`, `Reflect.*`, class behavior, and Proxy invariants |
| ES Module lifecycle (`parse -> instantiate -> evaluate`) | Modern JS projects expect standards-compliant module loading semantics | HIGH | Requires module records, resolver hooks, and error propagation compatible with spec |
| Promise and microtask job queue semantics | Async correctness is table stakes for modern JS execution | HIGH | Requires host job queue hooks, ordering guarantees, and exception propagation |
| Core built-ins coverage (`Object`, `Function`, `Array`, `String`, `Number`, `Boolean`, `Math`, `Date`, `Error`, `JSON`, `RegExp`, `Map/Set`, weak collections) | Users expect built-ins to work before advanced platform APIs | HIGH | Requires internal slots + descriptor correctness + iterator protocol + GC-aware object model |
| test262-driven compatibility pipeline with CI gates | Modern engines are expected to prove correctness against shared conformance baselines | MEDIUM | Depends on stable harness, reproducible snapshots, and clear skip policy |
| GC correctness + root strategy under stress | Long-running embedded hosts require memory safety and predictable reclamation behavior | HIGH | Requires robust root accounting (stack/globals/modules/jobs) and stale handle safety |
| Embeddable host API (library-first context/realm, host functions/modules, execution interrupt hooks) | Engine projects are expected to be integrable into larger hosts | MEDIUM | Keep host boundary explicit and minimal; avoid coupling runtime core to product-specific APIs |
| Debuggability (error locations, stack traces, bytecode/semantic diagnostics) | Teams need actionable failure signals to ship/maintain engine integrations | MEDIUM | Strongly improves velocity in compatibility bug closure and regression triage |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| QuickJS semantic parity program with explicit mapping/checklists and delta tracking | Reduces ambiguity by turning "compatible enough" into auditable, per-feature parity targets | MEDIUM | Build on existing docs (`quickjs-mapping`, `semantics-checklist`) and keep automated evidence in snapshots |
| Conformance-first governance (test262 clusters + GC guard baselines in CI) | Gives maintainers confidence that semantic and memory regressions are caught early | MEDIUM | Use existing harness momentum and make gate thresholds explicit per milestone |
| GC observability profiles (default, stress, runtime safety-point) | Enables production-like confidence and faster root-cause analysis for memory bugs | MEDIUM | Extend current GC stats/baselines into standardized perf+correctness dashboards |
| Rust-native host extensibility without runtime-core C FFI | Strong safety/portability story for Rust embedders and easier ownership reasoning | HIGH | Keep runtime core Rust-only; expose host extension points via traits/interfaces |
| Fail-loud semantic policy for unsupported forms | Avoids dangerous silent divergence and simplifies downstream debugging | LOW | Replace compatibility shortcuts with explicit syntax/runtime errors where unsupported |
| Modular crate boundaries with selective embedding options | Lets adopters use only needed layers (parser/harness/runtime subsets) and lowers integration cost | MEDIUM | Continue splitting VM hot spots into focused modules to reduce coupling and merge risk |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Early JIT/AOT tiering before semantic closure | Seen as the fastest path to benchmark wins | High complexity and regression risk while semantics and built-ins are still moving | Finish semantic closure and compatibility gates first, then add focused optimization workstreams |
| Full Node/Web API parity inside runtime core | Users want "drop-in runtime" convenience | Blurs engine-vs-host boundary and explodes scope (networking, timers, filesystem, policy) | Keep core as ECMAScript engine; provide optional host/runtime adapter crates |
| Silent parser/runtime fallbacks for unsupported language forms | Makes progress look fast in early phases | Creates hidden incompatibilities and hard-to-debug production behavior | Fail loudly with explicit `SyntaxError`/`TypeError` until proper semantics are implemented |
| Monolithic VM growth as a single-file implementation strategy | Feels faster for short-term iteration | Increases change coupling, review risk, and regression blast radius | Modularize VM by domain (objects/calls/gc/regexp/control-flow) with targeted tests |
| Unsafe-by-default memory tricks as primary strategy | Promises smaller/faster value representation quickly | Safety/debuggability cost is high without mature invariants and fuzz coverage | Keep safe baseline first; introduce optional, benchmark-validated optimizations behind flags |

## Feature Dependencies

```text
Core semantics (scope/closures/descriptors/eval)
    -> required by Built-ins correctness
        -> required by High test262 pass rates

ES Modules lifecycle
    -> requires Realm/environment model
    -> requires Module resolver hooks
    -> requires Promise job queue for async module paths

Promise and microtasks
    -> requires Host job executor hooks
    -> required by async language and runtime behavior

GC root strategy and handle safety
    -> required by long-running embedding stability
    -> enhances conformance reliability under stress modes

Library-first minimal core boundary
    -> conflicts with Full Node/Web API-in-core approach
```

### Dependency Notes

- **Built-ins correctness requires core semantics:** most built-ins exercise descriptor/prototype and completion semantics, so language-core bugs leak into built-ins quickly.
- **ES Modules require job queue integration:** async loading/evaluation and dynamic import behavior need a reliable microtask/job executor contract.
- **GC safety underpins everything stateful:** weak collections, closures, iterators, and module caches all rely on correct root management.
- **Conformance rate depends on feature ordering:** closing parser/vm/runtime semantic gaps before broad API expansion improves pass-rate efficiency.
- **Minimal core boundary conflicts with host-API sprawl:** mixing host platform APIs into core will slow milestone velocity and weaken maintainability.

## MVP Definition

### Launch With (v1)

Minimum viable product for the next milestone gate in this brownfield project.

- [ ] Language-core semantic closure for highest-impact gaps (`eval`/`with`/strict interactions, descriptor edges, object invariants)
- [ ] Spec-correct Promise job queue and microtask ordering
- [ ] ES Module parse/instantiate/evaluate path with host loader hooks
- [ ] Core built-ins behavior closure for current high-failure clusters
- [ ] GC stress stability gates (root correctness + baseline regression checks)

### Add After Validation (v1.x)

- [ ] Advanced diagnostics polish (improved traces, richer failure clustering) once semantic closure is stable
- [ ] Optional host runtime adapters (timers/fetch-like APIs) outside runtime core boundary
- [ ] Focused performance tuning on measured hotspots (regex, GC cadence, VM dispatch)

### Future Consideration (v2+)

- [ ] Optional optimization tier (JIT/AOT experiments) after conformance and API stability goals are met
- [ ] Broader platform conformance programs (e.g., larger Web API/WPT-aligned surfaces via adapters)
- [ ] Advanced parallel/shared-memory features when host threading model is ready

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Core semantics closure (`eval`/`with`/strict/descriptors) | HIGH | HIGH | P1 |
| Promise job queue + microtasks | HIGH | HIGH | P1 |
| ES Modules lifecycle | HIGH | HIGH | P1 |
| Built-ins closure for remaining major gaps | HIGH | HIGH | P1 |
| GC root/stress hardening | HIGH | HIGH | P1 |
| Conformance automation and failure clustering | HIGH | MEDIUM | P1 |
| Host adapter ecosystem (outside core) | MEDIUM | MEDIUM | P2 |
| Optimization-tier experiments (JIT/AOT) | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Competitor A | Competitor B | Our Approach |
|---------|--------------|--------------|--------------|
| Conformance signaling | QuickJS reports near-100% ES2023 test-suite pass and Annex B breadth | Boa reports 90%+ conformance and publishes release-level gains | Keep transparent test262 snapshots and failure-cluster burn-down in-repo |
| Embedding model | QuickJS exposes runtime/context/class C API and interrupt hooks | Boa emphasizes Rust embedding APIs and async job/module integration | Provide Rust-native host hooks with explicit boundaries and no runtime-core C FFI |
| Runtime scope | QuickJS keeps a small embeddable core plus minimal standard modules | Boa is adding optional runtime APIs (`fetch`, timers, `queueMicrotask`) | Keep engine core minimal; move host APIs into optional adapter crates |
| Memory/perf strategy | QuickJS highlights small footprint and deterministic GC behavior | Boa v0.21 moved `JsValue` default to NaN boxing and raised conformance/perf | Keep correctness-first baseline, then add benchmark-proven optimizations incrementally |

## Sources

- Local project context: `AGENTS.md`, `.planning/PROJECT.md`, `.planning/codebase/ARCHITECTURE.md`, `.planning/codebase/CONCERNS.md`, `docs/current-status.md`
- QuickJS documentation and feature claims: https://bellard.org/quickjs/quickjs.html
- QuickJS release/news page: https://www.bellard.org/quickjs/
- Boa repository README and crate/runtime/conformance references: https://github.com/boa-dev/boa
- Boa site summary (conformance posture): https://boajs.dev/
- Boa v0.21 release details (conformance delta, job executor/module loader/runtime APIs): https://boajs.dev/blog/2025/10/22/boa-release-21
- test262 official suite scope and role: https://github.com/tc39/test262
- Brimstone (Rust engine ecosystem comparator): https://github.com/Hans-Halverson/brimstone

---
*Feature research for: Pure Rust JavaScript runtime engine ecosystem*
*Researched: 2026-02-25*
