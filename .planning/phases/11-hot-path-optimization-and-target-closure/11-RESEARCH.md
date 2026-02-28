# Phase 11: Hot-Path Optimization and Target Closure - Research

**Researched:** 2026-02-28  
**Domain:** qjs-rs VM/runtime/bytecode hot-path optimization with benchmark-target closure (`qjs-rs <= boa-engine`)  
**Confidence:** HIGH (repo-grounded), MEDIUM (target-closure feasibility until hotspot attribution is completed)

## User Constraints

- Phase target is fixed: **Phase 11 — Hot-Path Optimization and Target Closure**.
- This phase must satisfy **PERF-03, PERF-04, PERF-05**.
- Roadmap success criteria require:
  1. Aggregate mean latency on tracked suite is no worse than `boa-engine` under same host/config.
  2. At least two hot paths (arith/array/call-heavy families) are optimized with before/after evidence.
  3. Optimizations include guard/fallback and preserve observable semantics.
  4. Runtime core remains pure Rust and layer boundaries stay maintainable.
- Project boundary remains active: **no runtime-core C FFI** (`AGENTS.md`).
- Priority order remains active: **semantic correctness > maintainability > performance** (`AGENTS.md`).
- Project discovery:
  - `CLAUDE.md`: not present.
  - `.agents/skills/`: not present.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PERF-03 | Aggregate mean latency on tracked suite is no worse than `boa-engine` on same host/config | Lock a single comparison profile + timing mode, generate fresh bench.v1 baseline, require aggregate + per-case delta report from same machine/config before closure claim |
| PERF-04 | At least two runtime hot paths receive targeted optimization with before/after evidence | Prioritize hotspots evidenced by current suite (`arith-loop`, `array-sum`, `fib-iterative`), implement two guarded optimization packets, and publish per-hotpath evidence (latency + semantic parity) |
| PERF-05 | Optimization changes preserve maintainable architecture boundaries and avoid major regressions | Enforce layer-local changes (`bytecode`, `vm`, `runtime`) with guard/fallback design, no runtime-core C FFI, and explicit architecture checklist for each optimization PR |
</phase_requirements>

## Summary

Phase 10 is complete and gives a valid `bench.v1` measurement contract, but current performance is still far from the Phase 11 target. A fresh local contract run on **2026-02-28** (`target/benchmarks/engine-comparison.local-dev.json`) shows `qjs-rs` aggregate mean at **1382.916 ms** vs `boa-engine` at **183.266 ms** (qjs-rs is **7.55x slower** on aggregate). The gap is concentrated in arithmetic/array/call-heavy families; `json-roundtrip` is already slightly faster than boa.

The current VM architecture explains this profile: hot execution paths are dominated by repeated dynamic lookups and conversions (identifier/property resolution via strings + map lookups, numeric coercion-heavy arithmetic paths, array index handling through string property keys). This means Phase 11 planning must be **hotspot-first and attribution-first**. Without instrumentation and staged optimization packets, it is easy to ship “fast but fragile” code and fail PERF-05.

**Primary recommendation:** Plan Phase 11 as 3 waves — **(W0) hotspot attribution + safety harness**, **(W1) first optimization packet (numeric/binding path)**, **(W2) second optimization packet (array/property path) + target-closure reruns** — with hard evidence gates per wave.

## Current Baseline Snapshot (for Planning)

Source: `target/benchmarks/engine-comparison.local-dev.json` (generated 2026-02-28, profile `local-dev`, `eval-per-iteration`, 200 iterations, 7 samples).

| Case | qjs-rs mean (ms) | boa mean (ms) | qjs-rs / boa |
|------|------------------:|--------------:|-------------:|
| arith-loop | 2091.660 | 203.949 | 10.256x |
| fib-iterative | 161.488 | 27.290 | 5.917x |
| array-sum | 3256.064 | 475.885 | 6.842x |
| json-roundtrip | 22.454 | 25.939 | 0.866x |
| **Aggregate** | **1382.916** | **183.266** | **7.546x** |

Planning implication: optimization should heavily target **arith-loop + array-sum + fib-iterative** families first. `json-roundtrip` should be treated as a semantic/perf non-regression canary.

## Standard Stack

### Core (must use)

| Component | Current Version/State | Purpose in Phase 11 | Why Standard Here |
|---|---|---|---|
| `crates/benchmarks` + `bench.v1` contract | in-repo (Phase 10 complete) | Single source of truth for before/after evidence | Already contract-normalized and accepted in verification |
| `crates/vm` + `crates/bytecode` | in-repo | Implement targeted fast paths/passes | Hot-path work is explicitly scoped to these layers by roadmap |
| Existing semantic gates (`cargo test`, test262-lite harness) | in-repo CI | Prove semantic non-regression for every optimization | Required by project priority and PERF-05 constraints |

### Supporting (use when evidence says needed)

| Tool/Lib | Current State | Use in Phase 11 |
|---|---|---|
| `tracing` / lightweight counters | not yet wired for perf attribution in VM | Add optional, low-overhead attribution for opcode family timing/counts |
| `criterion` / `iai-callgrind` (from research stack) | not currently integrated in repo | Optional deeper microbench/instruction evidence if macrobench deltas are ambiguous |
| `smallvec` / `rustc-hash` | not currently used in VM hot path | Only introduce after profiler evidence, not preemptively |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| Optimize VM/runtime hot paths | Benchmark-specific shortcuts in harness | Faster to implement but violates confidence/maintainability intent and risks invalid PERF-04 evidence |
| Guarded local optimizations | Large representation rewrite (NaN-boxing / runtime-wide refactor) | Potentially larger upside but too much phase risk vs PERF-05 boundary |
| Layer-local changes | Cross-layer “fast hack” shortcuts | Short-term speed, long-term architecture erosion |

## Architecture Patterns

### Recommended project structure for this phase

```text
crates/
├── bytecode/src/
│   ├── lib.rs                  # existing
│   └── opt/                    # add phase-11 pass/pattern helpers (small, focused)
├── vm/src/
│   ├── lib.rs                  # existing execution core
│   ├── perf/                   # optional: counters/snapshots for attribution
│   └── fast_path/              # optional: guarded hot-path helpers
└── benchmarks/src/
    └── main.rs                 # keep bench.v1 evidence flow; add optional attribution export
```

### Pattern 1: Optimization Packet (measure → optimize → prove)

**What:** Treat each hot-path optimization as a packet with its own evidence bundle.

**Packet minimum contents:**
1. Baseline numbers (before)
2. Code change (guarded fast path + fallback)
3. Semantic parity proof (tests)
4. Benchmark rerun (after)
5. Delta summary (per-case + aggregate)

**When to use:** Every PERF-04 hotspot.

### Pattern 2: Guarded Fast Path + Canonical Fallback

**What:** Fast path executes only under explicit safe preconditions; otherwise existing path runs unchanged.

**When to use:** Numeric ops, array indexed access, call dispatch paths.

**Example shape:**
```rust
if fast_path_preconditions_hold { 
    return fast_result;
}
// Existing spec-aligned path
slow_path(...)
```

### Pattern 3: Tiered wave plan to protect PERF-05

- **Wave 0:** Add attribution + parity harness first (no speed claim yet).
- **Wave 1:** First hotspot packet (e.g., numeric/binding).
- **Wave 2:** Second hotspot packet (e.g., array/property) + closure run.

This sequencing preserves rollback clarity and keeps changes auditable.

## Candidate Hot Paths (Evidence-Driven)

| Priority | Hot Path | Evidence | Candidate Optimization | Risk Level |
|---|---|---|---|---|
| P1 | Identifier/binding resolution in loops | `arith-loop`/`fib-iterative` large gaps; VM uses repeated name-based resolution in execute loop | Introduce faster local binding path (cached binding IDs / slot-like access where safe) | MEDIUM |
| P1 | Numeric arithmetic op path | Arithmetic-heavy case is worst (10.256x) and arithmetic ops go through coercion-heavy generic paths | Add guarded numeric fast path for common `Number`-`Number` operations with fallback | LOW-MEDIUM |
| P1 | Array indexed get/set path | `array-sum` is 6.842x slower; path converts indices via string/property machinery | Add guarded dense-index array fast path (or specialized indexed op handling) while preserving descriptor/prototype semantics | MEDIUM-HIGH |
| P2 | Call-heavy dispatch overhead | `fib-iterative` still 5.917x gap | Fast path for direct call patterns where callable target is already known and safe | MEDIUM |

## Don’t Hand-Roll

| Problem | Don’t Build | Use Instead | Why |
|---|---|---|---|
| “Quick win” benchmark improvements | Benchmark-specific script special-casing | Generic VM/bytecode hot-path optimizations | Prevents overfitting and preserves trust in PERF-04 evidence |
| Safety validation | Ad-hoc manual spot checks only | Automated parity reruns + targeted semantic tests | Required to avoid semantic drift under optimization |
| Architecture governance | Implicit “it’s small enough” decisions | Explicit optimization checklist (boundary, fallback, rollback) | Directly supports PERF-05 maintainability clause |

**Key insight:** Phase 11 success is not just lower numbers; it is **auditable lower numbers with preserved semantics and maintainable code boundaries**.

## Common Pitfalls (Phase-11 Critical)

### Pitfall 1: Chasing aggregate only
- **What goes wrong:** Aggregate improves while one family regresses.
- **Avoidance:** Require per-case guardrails in every optimization packet (`arith-loop`, `fib-iterative`, `array-sum`, `json-roundtrip`).

### Pitfall 2: Semantic drift in fast paths
- **What goes wrong:** Numeric/array/call shortcuts bypass coercion/prototype/error-order semantics.
- **Avoidance:** Guard/fallback by design + optimization on/off differential tests for each hotspot.

### Pitfall 3: Hidden architecture erosion
- **What goes wrong:** Hot path logic leaks across bytecode/vm/runtime boundaries.
- **Avoidance:** Keep each optimization owned by one primary layer; if cross-layer needed, separate refactor commit from optimization commit.

### Pitfall 4: Noisy benchmark decisions
- **What goes wrong:** micro fluctuations are treated as real wins.
- **Avoidance:** Use same host/profile/contract and report mean + median + stddev from bench.v1 artifacts.

## Code Examples (from current code, planning anchors)

### 1) VM dispatch choke point
`crates/vm/src/lib.rs` executes a large `match` on every opcode in `execute_code`, making opcode-path overhead a primary optimization target.

### 2) Arithmetic path currently generic
`Opcode::Add` routes through `evaluate_add`, which always performs primitive conversion logic before deciding numeric/string behavior.

### 3) Array indexing through property strings
Array read/write paths frequently pass through property key strings and object property maps, consistent with `array-sum` gap profile.

## Suggested Planning Slices (for PLAN.md)

1. **11-01 Attribution + Safety Harness (must start here)**
   - Add hotspot counters/stage attribution hooks.
   - Add optimization on/off parity harness for target scripts.
   - Produce baseline attribution artifact tied to `bench.v1` run.

2. **11-02 Optimization Packet A (numeric + binding path)**
   - Implement guarded numeric fast path and one binding-resolution acceleration path.
   - Add targeted regression tests (normal + edge/exception behavior).
   - Publish before/after benchmark evidence.

3. **11-03 Optimization Packet B (array/property path) + closure run**
   - Implement guarded array indexed access optimization.
   - Ensure fallback for prototype/accessor/non-dense cases.
   - Rerun full tracked suite and evaluate PERF-03 closure (`qjs-rs <= boa`).

## Open Questions (must be resolved in planning)

1. **What is the authoritative PERF-03 gate profile?**
   - Known: Contract supports `local-dev` and `ci-linux`; current measured baseline is local-dev (2026-02-28).
   - Unknown: Must closure be demonstrated on `local-dev`, `ci-linux`, or both?
   - Recommendation: Lock one primary closure profile in plan frontmatter and treat the other as corroborating evidence.

2. **How much structural change is acceptable under PERF-05?**
   - Known: Large rewrites are discouraged; maintainability boundaries are required.
   - Unknown: Whether introducing new VM submodules (`perf/`, `fast_path/`) is acceptable in this phase.
   - Recommendation: Approve a boundary checklist up front and require each plan slice to declare touched layers.

3. **Should comparator/version metadata be strengthened before target-closure claim?**
   - Known: Node/QuickJS metadata is explicit; boa in-process metadata is generic string.
   - Unknown: Whether boa exact version capture is mandatory for closure audit.
   - Recommendation: Add explicit boa version metadata in Phase 11 evidence to prevent baseline ambiguity.

4. **Can quickjs-c remain missing on primary dev host for PERF-03 closure?**
   - Known: Current 2026-02-28 local run marks quickjs-c as missing.
   - Unknown: Whether closure evidence requires all comparators available or only qjs-vs-boa equality.
   - Recommendation: Explicitly document comparator availability policy in Phase 11 plan acceptance notes.

## Sources

Primary (repo, HIGH confidence):
- `AGENTS.md`
- `.planning/REQUIREMENTS.md`
- `.planning/STATE.md`
- `.planning/ROADMAP.md`
- `.planning/research/SUMMARY.md`
- `.planning/research/STACK.md`
- `.planning/research/FEATURES.md`
- `.planning/research/ARCHITECTURE.md`
- `.planning/research/PITFALLS.md`
- `.planning/phases/10-baseline-contract-and-benchmark-normalization/10-VERIFICATION.md`
- `docs/benchmark-contract.md`
- `docs/engine-benchmarks.md`
- `crates/benchmarks/src/main.rs`
- `crates/benchmarks/src/contract.rs`
- `crates/bytecode/src/lib.rs`
- `crates/vm/src/lib.rs`
- `crates/runtime/src/lib.rs`
- `.github/workflows/ci.yml`

Fresh measurement artifact (HIGH confidence, generated during this research):
- `target/benchmarks/engine-comparison.local-dev.json` (generated 2026-02-28)
- `target/benchmarks/engine-comparison.local-dev.md`

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — based on current in-repo tooling and Phase 10 verified contract.
- Architecture patterns: **HIGH** — directly aligned with current crate boundaries and existing hot paths.
- Target closure feasibility: **MEDIUM** — current gap is large (7.55x), so success depends on real hotspot distribution and optimization depth discovered in Wave 0.

**Research date:** 2026-02-28  
**Valid until:** 2026-03-14 (refresh earlier if benchmark contract/timing profile changes)
