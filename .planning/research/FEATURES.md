# Feature Research

**Domain:** qjs-rs v1.1 JavaScript runtime performance acceleration milestone
**Researched:** 2026-02-27
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist for a serious runtime performance milestone. Missing these means optimization claims are not trusted.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Reproducible cross-engine benchmark harness (`qjs-rs`, `boa-engine`, `quickjs-c`, `nodejs`) | Performance milestones are only credible with consistent external baselines | MEDIUM | Maps to PERF-01; must emit machine-readable JSON + human-readable report artifacts |
| Representative hot-path benchmark suite (arith loop, call-heavy, array workload, JSON roundtrip) | "Faster" must reflect real runtime hotspots, not one synthetic micro-bench | MEDIUM | Maps to PERF-02; keep case set stable to preserve trend comparability |
| Hotspot attribution + before/after evidence for each optimization | High-quality milestones show where time moved, not just final score | MEDIUM | Pair profiling output with per-case delta table for every optimized path |
| Semantic non-regression safety net on every optimization iteration | Runtime users expect speedups without behavioral drift | HIGH | Maps to TST-05; `cargo test` + governance/test262-lite must stay green |
| CI performance regression gate with explicit thresholds | Performance work regresses quickly without automated guards | MEDIUM | Maps to TST-06; publish deterministic artifact paths and failure criteria |
| Architecture-bound optimization scope (bytecode/vm/runtime hot paths only) | Teams expect maintainable acceleration, not milestone-breaking rewrites | MEDIUM | Maps to PERF-05; no runtime-core C FFI, no semantic-layer destabilization |

### Differentiators (Competitive Advantage)

Features that can make qjs-rs v1.1 stand out while staying correctness-first.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Dual-gate workflow: performance delta + semantic delta in one PR contract | Makes optimization work auditable and safe; avoids "fast but wrong" merges | MEDIUM | Every perf PR should include benchmark diff and semantic gate evidence |
| Hot-path optimization playbook by layer (`bytecode -> vm -> runtime`) | Improves iteration speed and lowers regression blast radius | MEDIUM | Encodes repeatable patterns (dispatch tightening, allocation reduction, call-path trimming) |
| Stable benchmark evidence pack in-repo (JSON, markdown report, chart) | Lets maintainers compare results over time and across machines consistently | LOW | Already aligned with `docs/engine-benchmarks.md` + report pipeline |
| Semantics-preserving fast paths with explicit fallback/deopt paths | Captures real wins without changing observable behavior | HIGH | Fast path must route to generic spec-correct path when preconditions fail |
| Performance SLO anchored to external engine (`qjs-rs <= boa-engine` aggregate) | Clear market-facing milestone definition instead of vague "faster" language | HIGH | Maps to PERF-03; keep threshold tied to tracked case portfolio |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem attractive but are likely to hurt semantic stability in this milestone.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Benchmark-specific special-casing (recognize exact scripts/opcode shapes) | Produces quick headline wins | Overfits to suite, hides real-world regressions, and corrupts trust in metrics | Optimize generic execution paths backed by varied representative cases |
| Large value-representation rewrite (e.g., full NaN-boxing migration) inside v1.1 | Promises broad speedups | High semantic and memory-safety risk; too much blast radius for one milestone | Keep representation stable; pursue focused hot-path wins first, isolate representation experiments to v1.2+ |
| Relaxed semantics mode as default (skip checks/coercions/errors for speed) | Appears to improve benchmark numbers immediately | Violates QuickJS-aligned correctness contract and causes hard-to-debug behavior drift | Preserve strict semantics; only optimize with equivalent observable behavior |
| Mixing major feature expansion (new language surfaces) into perf milestone | "Ship more while touching runtime anyway" | Adds confounding variables, slows validation, and increases regression probability | Keep v1.1 scope on performance and governance only; defer feature expansion |
| Turning off semantic/governance gates in perf branches | Reduces short-term CI friction | Allows silent correctness regressions and invalidates performance conclusions | Keep gates mandatory; if needed split perf jobs (fast smoke + nightly full) but do not remove checks |

## Feature Dependencies

```text
Reproducible benchmark harness (PERF-01)
    -> required by Reliable hotspot attribution
        -> required by Targeted optimization evidence (PERF-04)
            -> required by Aggregate <= boa-engine claim (PERF-03)

Representative benchmark suite (PERF-02)
    -> required by Meaningful CI regression thresholds (TST-06)

Semantic non-regression gate (TST-05)
    -> required by Safe fast-path rollout

Architecture-bound optimization scope (PERF-05)
    -> constrains -> Large representation rewrite (anti-feature)

Benchmark special-casing (anti-feature)
    -> conflicts with -> Cross-engine credibility + reproducibility
```

### Dependency Notes

- **PERF-03 depends on PERF-01 + PERF-02 + PERF-04:** You cannot credibly claim parity/superiority vs boa-engine without stable baselines, representative workloads, and attributable optimizations.
- **TST-06 depends on stable suite definition:** Thresholds are only useful if benchmark cases/configuration are controlled and versioned.
- **Safe fast paths depend on TST-05:** Any optimization that bypasses generic logic must be continuously validated against semantic tests.
- **PERF-05 conflicts with large architectural rewrites:** v1.1 should improve existing layers, not destabilize them with broad representation/GC model changes.
- **Anti-feature special-casing conflicts with milestone goal:** It may improve one chart while reducing general runtime quality and long-term competitiveness.

## MVP Definition

### Launch With (v1)

Minimum viable v1.1 performance milestone scope.

- [ ] Cross-engine reproducible benchmark pipeline + artifacts (PERF-01)
- [ ] Representative hot-path benchmark portfolio with configurable run controls (PERF-02)
- [ ] At least two targeted, measured hot-path optimizations in VM/runtime/bytecode paths (PERF-04)
- [ ] Aggregate benchmark mean latency no worse than boa-engine on tracked suite (PERF-03)
- [ ] Semantic/governance gates and CI perf regression guardrails remain green and enforced (TST-05, TST-06)

### Add After Validation (v1.x)

Features to add once MVP performance closure is stable.

- [ ] Extend benchmark corpus (typed-array-heavy, object-shape churn, exception-heavy paths) — trigger: v1.1 parity sustained for 2+ cycles
- [ ] Introduce deeper profiling automation (flamegraph/per-case instruction counters) — trigger: current top hotspots reduced and next bottlenecks unclear
- [ ] Add optional experimental fast paths behind feature flags — trigger: baseline CI/perf governance proven reliable

### Future Consideration (v2+)

Features to defer until semantic/perf governance matures beyond v1.1.

- [ ] Value-representation overhaul (e.g., NaN-boxing or equivalent) — defer due to high semantic + memory-model risk
- [ ] Tiered execution (baseline interpreter + JIT/AOT experiments) — defer until stable optimization telemetry and correctness envelope exist
- [ ] GC strategy redesign (incremental/parallel tuning) — defer until current hotspot and allocation profile plateaus are demonstrated

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Reproducible cross-engine benchmark harness + artifacts | HIGH | MEDIUM | P1 |
| Representative hot-path suite + run controls | HIGH | MEDIUM | P1 |
| Targeted VM/runtime/bytecode hot-path optimizations | HIGH | HIGH | P1 |
| Semantic + governance non-regression enforcement during perf work | HIGH | MEDIUM | P1 |
| CI performance threshold gate and artifact publication | HIGH | MEDIUM | P1 |
| Optimization playbook + attribution instrumentation | MEDIUM | MEDIUM | P2 |
| Expanded benchmark corpus and profiling depth | MEDIUM | MEDIUM | P2 |
| Representation/JIT/GC redesign experiments | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Competitor A | Competitor B | Our Approach |
|---------|--------------|--------------|--------------|
| Baseline performance reference point | `quickjs-c` serves as strong low-latency interpreter reference in current benchmark reports | `boa-engine` is direct Rust-engine parity target for v1.1 | Keep both in every benchmark run, plus `nodejs`, so qjs-rs decisions are evidence-driven |
| Milestone success criterion | QuickJS(C) shows what optimized interpreter paths can achieve | Boa offers realistic near-term parity target for Rust runtime architecture | Use staged targeting: first `qjs-rs <= boa-engine`, then iterate toward QuickJS(C) gap closure |
| Perf-vs-correctness discipline | QuickJS value is tightly tied to semantic reliability | Boa comparability is meaningful only if semantics are preserved | Treat semantic gates as hard constraints; no perf-only shortcuts that alter observable behavior |

## Sources

- `.planning/PROJECT.md` (v1.1 goal, constraints, milestone context)
- `.planning/REQUIREMENTS.md` (PERF-01..05, TST-05..06)
- `docs/engine-benchmarks.md` (cross-engine benchmark workflow and artifacts)
- `docs/reports/engine-benchmark-report.md` (current baseline deltas vs `boa-engine`, `quickjs-c`, `nodejs`)
- `AGENTS.md` (project-wide priority ordering and runtime boundaries)

---
*Feature research for: qjs-rs v1.1 performance acceleration milestone*
*Researched: 2026-02-27*
