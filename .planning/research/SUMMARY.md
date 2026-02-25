# Project Research Summary

**Project:** qjs-rs
**Domain:** Pure Rust embeddable JavaScript runtime aligned with QuickJS semantics
**Researched:** 2026-02-25
**Confidence:** HIGH

## Executive Summary

qjs-rs is a language-engine product, not a host-platform runtime, and the research consistently points to a correctness-first build strategy used by mature engine teams: stable layered architecture (`parser -> bytecode -> vm -> runtime -> builtins`), strict conformance gates, and explicit compatibility tracking against shared baselines (test262 plus targeted QuickJS behavior diffing).

The recommended approach is to keep the current stable workspace boundaries and drive the next milestones through semantic closure before surface-area expansion. In practical terms, that means eliminating silent fallbacks, hardening object/descriptor invariants, landing Promise microtask semantics with host queue hooks, then implementing full ESM instantiate/evaluate flow before broad builtin expansion and performance tuning.

The largest risks are sequencing and signal-quality risks: planning from outdated project status, shipping placeholder async/module behavior, and over-trusting narrow pass-rate metrics. These are mitigated by dependency-ordered phases, dual-profile GC gates (default + stress), and roadmap KPIs that track executed coverage and skip-bucket reduction, not just failure counts.

## Key Findings

### Recommended Stack

The current stack is already the right foundation for this project: stable Rust (Edition 2024), Cargo workspace modularity, strict CI gates (`fmt`, `clippy -D warnings`, `test`), and conformance-first validation with test262 plus QuickJS comparison. This directly matches the project's hard constraints (pure Rust runtime core, semantics first).

Recommended additions are selective and phase-gated: property/snapshot testing (`proptest`, `insta`) for semantic hardening, structured reporting (`serde/serde_json`) for compatibility and GC telemetry, and `criterion` for later performance work. None of these should displace core semantic gates.

**Core technologies:**
- Rust stable (Edition 2024): runtime core implementation - aligns with safety, maintainability, and current CI baseline.
- Cargo workspace (`resolver = 2`): modular engine evolution - preserves clear crate boundaries and incremental delivery.
- test262 + QuickJS behavioral diffing: semantic verification - provides auditable compatibility signals.
- GitHub Actions quality gates: continuous enforcement - prevents silent regressions in semantics and GC behavior.

### Expected Features

Research converges on five launch-critical capabilities: core language semantic closure, correct object/prototype/descriptor behavior, Promise microtask semantics, ESM lifecycle support, and GC/root correctness under sustained execution. These are table stakes for an embeddable JS engine.

**Must have (table stakes):**
- Core semantics closure (`eval`/`with`/strict mode interactions, closures, exceptions) - users expect correct language behavior.
- Object model and descriptor invariants - required for almost all builtin and userland correctness.
- Promise microtask queue semantics - required for modern async behavior.
- ES Module lifecycle (`parse -> instantiate -> evaluate`) - expected in modern JS usage.
- Builtins correctness for core globals (`Object`, `Function`, `Array`, `Error`, `JSON`, etc.) - required for practical compatibility.
- GC root strategy and safety under default + stress profiles - required for embeddability and stability.

**Should have (competitive):**
- Explicit QuickJS parity tracking (mapping/checklist + deltas) - clearer compatibility governance.
- Strong diagnostics (error locations, stack traces, bytecode diagnostics) - faster triage and integration.
- Rust-native host hook APIs for jobs/modules - better embedding ergonomics without violating core boundaries.

**Defer (v2+):**
- JIT/AOT optimization tiers - high complexity before semantic closure.
- Node/Web API parity inside runtime core - scope blow-up and boundary erosion.
- Aggressive unsafe/NaN-boxing-first redesign - premature until invariants and conformance stabilize.

### Architecture Approach

The preferred architecture remains layered and explicit: frontend and compilation feed a VM semantic engine backed by runtime state (values, objects, realms), with GC/observability as a first-class layer. Promise jobs and ESM must be treated as explicit subsystems with host-boundary traits, not implicit side effects.

**Major components:**
1. Frontend + compiler (`lexer/parser/bytecode`) - parses source and emits stable VM contract.
2. VM semantic engine (`exec/env/call`) - executes opcodes and ECMAScript operations.
3. Runtime state (`JsValue`, object model, descriptors, realms) - owns canonical semantic data model.
4. Async/module subsystems (`jobs`, `modules`, host hooks) - enforces Promise ordering and ESM lifecycle.
5. Memory + observability (`gc`, root manager, telemetry) - guarantees correctness under long-running workloads.

### Critical Pitfalls

1. **Outdated-baseline planning** - always gate roadmap phases against `docs/current-status.md` and active concern files.
2. **Silent semantic fallbacks** - replace fallback behavior with explicit parse/runtime errors plus regression tests.
3. **Promise/module sequencing drift** - land job queue + host hooks before major async/builtin expansion.
4. **GC blind spots in default profile** - keep dual-profile correctness gates (default + stress), not stress-only checks.
5. **Monolithic VM coupling** - split VM domains before heavy Phase 6/7 feature waves.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Brownfield Re-baseline and Semantic Hardening
**Rationale:** current codebase already passed scaffold stage; immediate value is closing high-impact semantic gaps.
**Delivers:** status-linked gap inventory, removal of silent fallbacks, explicit unsupported-path errors, targeted semantic regressions.
**Addresses:** core semantics closure and object invariant correctness.
**Avoids:** outdated-baseline planning and silent-fallback pitfalls.

### Phase 2: VM Domain Modularization and Descriptor Integrity
**Rationale:** reducing VM coupling lowers regression blast radius before async/module foundations.
**Delivers:** VM split by ownership (`exec/env/call/gc/regexp`), descriptor invariant test pack, clearer module boundaries.
**Uses:** existing Rust workspace + strict CI + snapshot/property testing additions.
**Implements:** architecture separation between runtime data model and semantic execution paths.

### Phase 3: Promise Job Queue Foundation
**Rationale:** async semantics are table stakes and a prerequisite for broader compatibility expansion.
**Delivers:** spec-ordered microtask queue, host enqueue/drain hooks, Promise settlement ordering regressions.
**Uses:** runtime host boundary patterns and conformance harness gates.
**Implements:** explicit jobs subsystem.

### Phase 4: ES Module Lifecycle and Loader Contract
**Rationale:** module support depends on stable realm + job queue behavior.
**Delivers:** parse/instantiate/evaluate lifecycle, module graph cache, loader error propagation and cyclic tests.
**Uses:** host resolver interface and VM/runtime module boundaries.
**Implements:** explicit modules subsystem.

### Phase 5: Builtins De-alias and Compatibility Expansion
**Rationale:** constructor alias debt blocks high-value conformance gains.
**Delivers:** de-alias schedule execution (WeakMap/WeakSet/typed arrays priorities), internal slot correctness, directory-level test262 closure targets.
**Uses:** declarative builtin/descriptor tables.
**Implements:** builtin surface completion with stable semantics.

### Phase 6: GC Robustness and Coverage Governance
**Rationale:** long-run stability and trustworthy metrics are release-critical.
**Delivers:** default+stress invariant gates, stale-handle regression suite, discovered/executed/skip KPI reporting, nightly stability policy.
**Uses:** structured harness telemetry and CI governance.
**Implements:** memory/observability hardening and compatibility signal quality controls.

### Phase Ordering Rationale

- Async and module work is intentionally sequenced after VM modularization to avoid compounding regressions in a monolithic core.
- Builtins expansion is sequenced after Promise/ESM foundations because many failures are dependency-driven, not isolated API bugs.
- GC and coverage governance are treated as release gates, ensuring pass-rate improvements represent real semantic closure.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3:** Promise job ordering edge cases and host scheduler contract details.
- **Phase 4:** ESM cyclic dependency semantics, loader failure modes, and top-level async interactions.
- **Phase 5:** High-risk builtin internal-slot behavior and de-alias migration sequencing.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Re-baseline + fallback removal is direct from existing status/concern docs.
- **Phase 2:** VM modularization and descriptor hardening follow established engine refactor patterns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core choices already validated by current repo and CI; only optional additions are medium-risk. |
| Features | HIGH | Table-stakes set is consistent across local research and external engine practice. |
| Architecture | HIGH | Layered boundaries and explicit jobs/modules map cleanly to current structure and known gaps. |
| Pitfalls | HIGH | Risks are directly evidenced in current status and concern patterns. |

**Overall confidence:** HIGH

### Gaps to Address

- Host API contract granularity: define exact trait surface and lifecycle guarantees during Phase 3/4 planning.
- Regex safety/performance policy: formalize timeout/budget strategy before broader untrusted-input scenarios.
- Builtin completion criteria: publish per-constructor exit checks to prevent alias debt recurrence.
- Performance baseline scope: lock benchmark corpus only after semantic gates stabilize.

## Sources

### Primary (HIGH confidence)
- `.planning/research/STACK.md` - recommended stack, tooling, constraints.
- `.planning/research/FEATURES.md` - feature prioritization and dependency map.
- `.planning/research/ARCHITECTURE.md` - architecture patterns, component boundaries, build order.
- `.planning/research/PITFALLS.md` - risk catalog and prevention/recovery strategies.
- `docs/current-status.md` - current brownfield status baseline for roadmap sequencing.

### Secondary (MEDIUM confidence)
- QuickJS docs and release notes - reference for semantic/embedding expectations.
- Boa project docs and release notes - comparative pure-Rust engine architecture and conformance trajectory.
- test262 project docs - conformance-suite role and scope.

### Tertiary (LOW confidence)
- Ecosystem comparator discussions (non-primary summaries) - directional context only; validate before policy changes.

---
*Research completed: 2026-02-25*
*Ready for roadmap: yes*
