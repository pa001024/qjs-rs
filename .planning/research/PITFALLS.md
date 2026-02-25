# Pitfalls Research

**Domain:** Pure Rust JavaScript runtime library aligned with QuickJS semantics (brownfield)
**Researched:** 2026-02-25
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Planning From Outdated Baseline Instead of Current Brownfield State

**What goes wrong:**
Roadmap phases repeat already-finished scaffolding and miss current blockers (modules, microtasks, semantic edge hardening), wasting milestone capacity.

**Why it happens:**
Phase descriptions in strategy docs look linear, but real progress has moved far beyond early-phase assumptions.

**How to avoid:**
Use `docs/current-status.md` and `.planning/codebase/CONCERNS.md` as mandatory inputs for each new milestone. Require every phase to declare which current gaps it closes and which already-complete work it depends on.

**Warning signs:**
- New phases still include workspace/bootstrap tasks.
- Milestone goals do not reference current test262 snapshots.
- Tasks mention “start parser/vm chain” even though chain is already running.

**Phase to address:**
Milestone planning gate before Phase 1 work starts (applies to every new milestone).

---

### Pitfall 2: Silent Semantic Fallbacks Mask Real Unsupported Behavior

**What goes wrong:**
Unsupported syntax/semantic paths (especially loop lowering edge cases) degrade into no-op or wrong runtime behavior instead of explicit failure.

**Why it happens:**
Interim compatibility shortcuts were added to keep execution green while semantics were incomplete.

**How to avoid:**
Eliminate silent fallbacks in parser/bytecode and replace with explicit early errors when behavior is not implemented. Track every temporary fallback with owner + deadline.

**Warning signs:**
- “Unsupported shape” paths compile to always-false loops.
- Pass rates improve without corresponding semantic implementation notes.
- New edge-case tests fail as “unexpected success” or skipped execution.

**Phase to address:**
Phase 1-2 hardening track, before broad Phase 7 expansion.

---

### Pitfall 3: Promise and Module Work Sequenced Too Late or Inverted

**What goes wrong:**
Builtins/async features expand on top of placeholder Promise semantics and no full job queue/module lifecycle, causing expensive rewrites once proper ordering semantics are introduced.

**Why it happens:**
Local feature wins (individual builtins) are easier to ship than foundational async runtime architecture.

**How to avoid:**
Treat “Promise job queue + host callback contract + module instantiate/evaluate flow” as prerequisites for any major async/builtin expansion. Make this a blocking milestone gate.

**Warning signs:**
- Promise constructor still behaves as placeholder in core paths.
- test262 module/strict/include-dependent suites remain broadly skipped.
- Async-related failures cluster around execution ordering/reentrancy.

**Phase to address:**
Phase 6 should start before second-wave builtin expansion and before Phase 7 full compatibility push.

---

### Pitfall 4: GC Correctness Regressions Hidden by Stress-Only Validation

**What goes wrong:**
GC appears healthy under stress profile snapshots, but production/default profile root-lifetime bugs remain (stale handles, shadow-root restore mismatches, rare UnknownObject failures).

**Why it happens:**
Stress mode catches throughput pressure, but lifecycle correctness bugs often appear in mixed/default execution and uncommon control-flow paths.

**How to avoid:**
Keep dual-profile gates: default correctness invariants + stress reclamation invariants. Add targeted regression suites for caller-state restore, unwind paths, and handle generation reuse.

**Warning signs:**
- Intermittent panics around caller shadow-root expectations.
- GC stats drift between default and stress without code changes.
- Fixes rely on ad hoc root pinning without invariant tests.

**Phase to address:**
Phase 4 continuation with carry-over gates into Phase 7 nightly stability.

---

### Pitfall 5: Builtin Constructor Aliasing Becomes Permanent Technical Debt

**What goes wrong:**
WeakMap/WeakSet/typed-array families remain wired to baseline constructors, producing deep semantic drift (internal slots, key constraints, coercion, iteration behavior).

**Why it happens:**
Alias wiring provides fast green-path progress and can survive too long if not explicitly retired.

**How to avoid:**
Create a de-alias schedule with per-constructor completion criteria (internal slots, brand checks, descriptor behavior, test262 directory targets). Ban new alias shortcuts unless temporary with expiry.

**Warning signs:**
- Multiple globals intentionally mapped to one constructor path.
- “Passes smoke tests” but fails directory-specific test262 semantics.
- Adding one builtin breaks another due shared internals.

**Phase to address:**
Phase 5 first half, before broadening language surface further.

---

### Pitfall 6: Monolithic VM File Prevents Safe Parallel Evolution

**What goes wrong:**
Core runtime work accumulates in a ~22k-line file, increasing merge conflicts, review blind spots, and cross-feature regressions.

**Why it happens:**
Fast semantic convergence favored single-file edits; modularization kept getting deferred.

**How to avoid:**
Schedule a structural refactor milestone: split VM domains (`call`, `objects`, `gc`, `regexp`, builtin wiring) with ownership boundaries and per-module tests before adding major new subsystems.

**Warning signs:**
- High conflict rate on `crates/vm/src/lib.rs`.
- “Small” fixes require large unrelated diff context.
- Regression root cause analysis repeatedly crosses unrelated runtime areas.

**Phase to address:**
Late Phase 3 / early Phase 5 as an enabling refactor before heavy Phase 6-7 work.

---

### Pitfall 7: Compatibility Metrics Look Green While Coverage Ceiling Stays Low

**What goes wrong:**
Pass percentage looks strong on sampled/allowed suites, but large semantic surfaces remain unexecuted due skip policy (`module`, `onlyStrict`, includes/feature flags).

**Why it happens:**
Progress reporting emphasizes passed/failed counts without enough attention to discovered-vs-executed ratios and skip categories.

**How to avoid:**
Promote coverage-execution ratios to first-class milestone KPIs. For each milestone, require explicit reduction of skip buckets, not just lower failure counts.

**Warning signs:**
- Executed case count plateaus while discovered grows.
- “0 failures” appears only on constrained subsets.
- Missing host features are tracked as skips for multiple milestones.

**Phase to address:**
Phase 7 planning and reporting governance (with prerequisites from Phase 6).

---

### Pitfall 8: Panic-Based Invariants Leak Into Public Runtime Surface

**What goes wrong:**
Invariant assumptions (`expect`, `unreachable`, panic-driven argument handling) can crash embedders under unexpected input paths.

**Why it happens:**
Internal assertions remain in production paths as implementation velocity outruns hardening.

**How to avoid:**
Convert reachable panic sites to typed compile/runtime errors; isolate hard panics to truly impossible states. Add fuzz/property tests around parser->bytecode->vm boundaries.

**Warning signs:**
- User-controlled inputs can trigger process abort.
- Bug reports include panic backtraces instead of JS/Rust error values.
- New invariants are added without negative tests.

**Phase to address:**
Phase 3 reliability hardening and Phase 7 robustness gate.

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Alias missing builtins to existing constructors | Fast compatibility progress | Deep semantic drift and rewrite cost | Only with explicit deprecation deadline |
| Silent parser fallback for unsupported forms | Keeps pipeline green | Hidden behavior mismatch and false confidence | Never |
| Keep all VM logic in single file | Low coordination cost initially | High coupling, conflict-heavy delivery | Temporary only during short stabilization windows |
| Manual builtin metadata tables | Quick feature increments | Descriptor/attribute mismatches and omission risk | Acceptable only with generated validation checks |

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| QuickJS semantic alignment workflow | Translating C implementation details literally | Align observable semantics first; allow Rust-native internals |
| test262 harness integration | Using subset pass rate as readiness signal | Track discovered/executed/failed/skip buckets together |
| Host embedding API | Letting engine panics propagate to host process | Return typed engine errors; isolate fatal faults |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Runtime GC stress knobs reused in normal runs | 10x-50x latency growth and noisy stats | Separate stress guard profile from default profile | Any sustained script workload |
| Regex compile-on-match path | Repeated regex-heavy code slows unpredictably | Cache compiled regex by `(pattern, flags)` | Medium-size regexp workloads |
| `fancy-regex` worst-case backtracking | Latency spikes / potential DoS | Add time or step budgets and safe fallbacks | Adversarial patterns/inputs |
| Per-case thread spawn in harness | High overhead in large suites | Use worker pool with panic isolation boundaries | Thousands of cases/nightly runs |

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Panic on malformed external input | Process-level denial of service | Replace panic paths with structured errors and stable exit codes |
| Unbounded regex execution | Algorithmic complexity attacks | Enforce regex budget/timeout; reject risky patterns in untrusted contexts |
| Missing resource limits in host hooks | Host starvation or runaway execution | Add execution/queue/memory budgets and host interrupts |

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Partial features without explicit capability signaling | Users cannot tell supported semantics boundary | Expose feature flags and compatibility matrix per release |
| Silent unsupported syntax behavior | Debugging becomes expensive and confusing | Fail fast with actionable syntax/runtime errors |
| Inconsistent error typing/messages | Hard to integrate in production tooling | Stabilize error taxonomy and map internals to user-facing errors |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Promise support:** Often missing real settlement + microtask ordering - verify queue semantics with ordering tests.
- [ ] **Module support:** Often missing instantiate/evaluate lifecycle - verify cyclic dependency and live binding behavior.
- [ ] **GC stability:** Often missing default-profile invariants - verify no stale-handle/unknown-object regressions in long runs.
- [ ] **Compatibility health:** Often missing discovered-vs-executed transparency - verify skip buckets shrink milestone over milestone.
- [ ] **Builtin completeness:** Often missing descriptor/internal-slot edge semantics - verify per-directory test262 closure, not just aggregate pass rate.

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Outdated-baseline roadmap | MEDIUM | Re-baseline from `current-status`, cancel duplicate phases, re-sequence by dependency |
| Silent fallback shipped | HIGH | Replace fallback with explicit error, backfill regression tests, rerun affected test262 directories |
| Promise/module sequencing error | HIGH | Freeze async feature additions, implement queue+module foundations, then replay regressions |
| GC lifecycle regression | HIGH | Reproduce with dual-profile harness, audit root ownership transitions, add targeted invariant tests before reopen |
| Builtin alias debt explosion | MEDIUM | Prioritize de-alias by highest semantic risk (WeakMap/WeakSet/typed arrays), lock alias creation policy |
| Monolithic VM merge instability | MEDIUM | Branch-protect refactor plan, split modules incrementally with test parity checkpoints |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Outdated baseline planning | Milestone planning gate (pre-Phase 1) | Every phase links to current gap IDs and latest snapshots |
| Silent semantic fallbacks | Phase 1-2 hardening | No unsupported path compiles silently; explicit error tests pass |
| Promise/module sequencing inversion | Phase 6 foundation | Promise ordering and module lifecycle suites run with expected semantics |
| GC correctness blind spots | Phase 4 + Phase 7 stability | Default + stress GC invariants both green in CI/nightly |
| Builtin constructor alias debt | Phase 5 early | Alias inventory trends to zero for high-risk globals |
| VM monolith coupling | Late Phase 3 / early Phase 5 refactor | `vm/lib.rs` size and conflict hotspots decrease; module tests isolated |
| Coverage illusion from skips | Phase 7 governance | Discovered/executed ratio and skip buckets improve each milestone |
| Panic leakage to embedders | Phase 3 reliability + Phase 7 robustness | Panic sites in public paths replaced by typed errors; fuzz regressions green |

## Sources

- `.planning/PROJECT.md`
- `.planning/codebase/CONCERNS.md`
- `docs/current-status.md`
- `AGENTS.md`

---
*Pitfalls research for: pure Rust JavaScript runtime aligned with QuickJS semantics*
*Researched: 2026-02-25*
