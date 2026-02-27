# Pitfalls Research

**Domain:** qjs-rs v1.1 performance acceleration for a semantics-stable pure Rust JS runtime
**Researched:** 2026-02-27
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Apples-to-Oranges Benchmark Contract Across Engines

**What goes wrong:**
`qjs-rs` appears faster or slower for the wrong reasons because each engine is measured under a different execution model (e.g., one re-parses/re-evals every iteration while another precompiles once).

**Why it happens:**
Cross-engine benchmark harnesses often drift into implementation-convenient paths instead of equivalent semantic paths.

**How to avoid:**
Define a benchmark contract before tuning: parse/compile scope, warmup policy, iteration semantics, and output validation must be equivalent for `qjs-rs`, `boa-engine`, `quickjs-c`, and `nodejs`. Require per-case checksum parity and explicit “compile-included vs execute-only” modes.

**Warning signs:**
- Large delta between "execute-only" and "eval-per-iteration" numbers.
- Performance claims based only on aggregate mean.
- Comparator engines use different code paths (`eval` loop vs reusable callable) without documentation.

**Phase to address:**
Phase 10 (PERF-01, PERF-02 baseline contract and harness normalization).

---

### Pitfall 2: Benchmark Noise Mistaken for Real Speedup

**What goes wrong:**
Optimization decisions are made from noisy samples (thermal throttling, scheduler jitter, background load), producing false wins and flaky CI gates.

**Why it happens:**
Teams add thresholds before stabilizing run conditions and variance budgets.

**How to avoid:**
Capture environment metadata in every report (CPU, governor, toolchain, engine versions), enforce minimum sample size, and gate on median/p95 + variance bounds instead of a single mean number.

**Warning signs:**
- Same commit swings >5-10% across reruns on same machine.
- Stddev is high but ignored in acceptance decisions.
- CI fails/pass alternates with no code changes.

**Phase to address:**
Phase 10 for reproducibility policy; Phase 12 for CI variance-aware threshold logic.

---

### Pitfall 3: Optimizing Aggregate Score While Regressing Key Workloads

**What goes wrong:**
Aggregate mean improves, but one critical workload (array/object semantics, call-heavy paths, JSON path) regresses materially.

**Why it happens:**
Single leaderboard metric hides per-case regressions; weighted mix is undefined.

**How to avoid:**
Track dual gates: (1) aggregate target (`qjs-rs <= boa-engine`) and (2) per-case non-regression bands. Make case-level deltas first-class in PR evidence.

**Warning signs:**
- “Overall faster” PRs with one or more cases >10% slower.
- Repeated optimization on one microbench family only.
- Hot-path tuning notes mention only total score.

**Phase to address:**
Phase 11 (optimization acceptance criteria) and Phase 12 (per-case CI thresholds).

---

### Pitfall 4: Semantic Drift From Fast Paths That Bypass Spec Operations

**What goes wrong:**
Fast paths skip observable JS semantics (`ToNumber`, `valueOf`/`toString` effects, hole handling, exception ordering), creating silent conformance regressions.

**Why it happens:**
After semantic closure, optimization work tends to assume monomorphic primitives and forgets dynamic object behavior.

**How to avoid:**
Require fast-path guard design docs: guard conditions, deopt fallback, and semantic equivalence tests for side-effectful/coercion-heavy edge cases.

**Warning signs:**
- Benchmarks improve but test262-lite edge suites dip.
- New opcode shortcuts bypass existing helper paths that encoded semantics.
- Bugs reported as “only wrong when objects override coercion hooks”.

**Phase to address:**
Phase 11 (hot-path implementation with semantic guardrails), validated again in Phase 12 with full non-regression gates.

---

### Pitfall 5: Invalid Inline Caches / Hidden-Class Assumptions in a Dynamic Runtime

**What goes wrong:**
Property access/call caches become stale after prototype/shape mutation, returning stale values or wrong method targets.

**Why it happens:**
Caching is introduced without robust invalidation and without mutation-heavy regression tests.

**How to avoid:**
Introduce versioned shape/prototype invalidation strategy first, then cache. Add dedicated mutation stress tests (prototype swaps, defineProperty changes, accessor transitions).

**Warning signs:**
- Heisenbugs that disappear when cache is disabled.
- Wrong behavior after `Object.setPrototypeOf` or descriptor redefinition.
- Fixes rely on broad cache flushes that erase wins.

**Phase to address:**
Phase 11 (cache design + invalidation), with Phase 12 soak runs for mutation scenarios.

---

### Pitfall 6: CPU Wins That Secretly Increase GC/Allocation Cost

**What goes wrong:**
Interpreter loop looks faster, but allocation churn or longer object lifetimes increase GC pause time and hurt end-to-end latency.

**Why it happens:**
Optimization reviews look at CPU-only microbench numbers and ignore allocation/GC telemetry.

**How to avoid:**
Every optimization PR must include allocation count and GC event deltas on benchmark cases. Reject wins that shift cost into GC unless total latency still improves within budget.

**Warning signs:**
- Throughput improves on short runs but worsens on longer iteration counts.
- GC frequency/pause duration rises after a “speedup” PR.
- Memory usage trends upward across samples.

**Phase to address:**
Phase 11 (optimization evidence requirements), Phase 12 (nightly trend regression checks).

---

### Pitfall 7: Architecture Erosion From Cross-Layer Micro-Optimizations

**What goes wrong:**
Performance patches punch through boundaries (`bytecode -> vm -> runtime -> builtins`), creating tightly coupled fast paths that are hard to maintain and risky to evolve.

**Why it happens:**
Short-term speed pressure rewards local hacks over systemic design.

**How to avoid:**
Add a “performance change architecture checklist” to PR review: boundary touched, API leakage, rollback plan, and ownership approval. Keep optimization localized or explicitly refactor layer contracts first.

**Warning signs:**
- Fast-path code duplicates semantics across crates.
- Large optimization diffs combine unrelated layers.
- Review comments repeatedly mention “temporary shortcut”.

**Phase to address:**
Phase 11 (PERF-05 maintainability boundary enforcement).

---

### Pitfall 8: CI Thresholds That Are Either Too Brittle or Too Loose

**What goes wrong:**
Brittle gates block healthy PRs due to normal noise; loose gates allow meaningful regressions to ship.

**Why it happens:**
Thresholds are set once from a single run and treated as universal truth.

**How to avoid:**
Use rolling baseline windows, per-case guard bands, and explicit rerun policy. Separate PR gate (coarse) from nightly gate (strict + trend-based). Store threshold rationale with owner and expiry review date.

**Warning signs:**
- Frequent manual retries to “get green”.
- Threshold updates happen ad hoc without rationale.
- Regressions discovered only after merge.

**Phase to address:**
Phase 12 (TST-06 governance and threshold lifecycle policy).

---

### Pitfall 9: Baseline Drift From Unpinned Comparator Versions

**What goes wrong:**
`boa-engine`, `nodejs`, `quickjs-c`, or toolchain updates change performance baseline independently of qjs-rs changes, confusing trend interpretation.

**Why it happens:**
External engine/tool versions are not pinned and not recorded as contract inputs.

**How to avoid:**
Pin comparator versions in benchmark metadata + docs, and treat version bumps as explicit baseline reset events requiring before/after dual-run evidence.

**Warning signs:**
- Sudden benchmark shift with no meaningful qjs-rs code change.
- Reports omit comparator and rustc versions.
- Historical trend breaks without migration notes.

**Phase to address:**
Phase 10 (baseline metadata contract) and Phase 12 (baseline reset governance).

---

### Pitfall 10: Verification Blind Spot — Performance Evidence Without Semantic Counter-Evidence

**What goes wrong:**
A PR is accepted as “faster” with benchmark artifacts, but semantic/governance suites are narrowed, skipped, or not tied to optimization diffs.

**Why it happens:**
Performance milestones bias review toward speed charts and away from full correctness matrix.

**How to avoid:**
Make optimization acceptance contingent on both: benchmark deltas and unchanged/expanded semantic gates (`cargo test`, lint/fmt, test262-lite governance, targeted edge regressions). Block merges if either side is missing.

**Warning signs:**
- “Perf-only” PRs without semantic non-regression artifact links.
- CI workflow adds benchmark step but weakens existing correctness gates.
- New fast paths lack dedicated edge-case tests.

**Phase to address:**
Phase 12 (dual-gate merge policy) with Phase 11 test additions per optimized hotspot.

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode benchmark scripts to a narrow microbench set | Quick scoreboard movement | Overfits optimizer to synthetic patterns, misses real workloads | Only as temporary smoke suite, never as sole gate |
| Add cross-layer "fast helper" bypassing existing semantic helpers | Immediate latency drop | Semantic drift + duplicated logic across crates | Only with explicit deopt + removal ticket |
| Tune with magic constants (cache sizes/thresholds) without telemetry | Easy local improvements | Unexplainable regressions on different machines | Acceptable only if constants are surfaced and benchmarked across profiles |
| Disable expensive checks in release for speed without alternative guardrails | Better benchmark numbers | Hard-to-debug correctness failures in production | Almost never; only with equivalent invariant checks elsewhere |

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `boa-engine` comparator | Compare against a floating crate version | Pin version + record in report metadata; bump via explicit baseline-reset PR |
| `quickjs-c` comparator via WSL/path | Assume fixed local path and shell behavior | Parameterize executable path and validate availability in harness preflight |
| `nodejs` comparator | Use host-global Node version without capture | Record Node version per run and fail fast if missing or unexpected |
| GitHub Actions runners | Treat cloud runner numbers as reproducible perf truth | Use CI for coarse non-regression; keep authoritative baseline on controlled runner/nightly |
| Existing governance pipeline | Add perf gates that bypass semantic gates | Keep performance checks additive; never replace fmt/clippy/test/test262-lite gates |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-parsing/re-compiling inside timed loop for some engines but not others | Misleading winner/loser ordering | Separate compile-cost and execute-cost modes consistently for all engines | Immediately once script complexity grows beyond trivial |
| Cold-cache measurements presented as steady-state wins | First run faster/slower than subsequent runs, noisy medians | Warmup policy + discard first N samples | Usually visible at 5+ samples per case |
| Single aggregate KPI gate | Hidden regressions in one workload family | Add per-case guardrails and weighted aggregate policy | Common once suite has 4+ heterogeneous cases |
| Optimizing tiny loops while ignoring allocation-heavy paths | CPU microbench improves, end-to-end latency stagnates | Include allocation/GC telemetry and long-run cases | Breaks on JSON/object-heavy workloads and longer runs |
| Thresholds calibrated on one machine profile | PR flakiness and repeated reruns | Calibrate by environment class and store guard bands | Breaks when runner class/toolchain changes |

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Removing runtime limits to improve benchmark numbers | Potential CPU/memory exhaustion under hostile scripts | Keep configurable execution/resource limits and exclude them only in explicit benchmark mode |
| Introducing unchecked arithmetic/unsafe assumptions in fast paths | Panic or undefined behavior risk in edge inputs | Preserve safe Rust semantics; add overflow and edge-case tests on optimized paths |
| Caching without invalidation under user-controlled prototype mutation | Incorrect execution that can bypass expected checks | Implement robust invalidation + mutation fuzz tests |

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Performance gate fails without reproducibility context | Contributors cannot debug why CI failed | Attach benchmark JSON + environment metadata artifact on every perf gate failure |
| Metric definitions change silently between phases | Team cannot compare trends across time | Version benchmark schema/metric definitions and announce changes in roadmap notes |
| “Beat boa” headline without per-case detail | Misaligned optimization effort and trust erosion | Publish per-case table + aggregate + variance in every milestone report |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Benchmark baseline:** Often missing fairness contract — verify compile/execute equivalence across all engines.
- [ ] **Optimization PR:** Often missing semantic edge tests — verify coercion/prototype mutation/error-order regressions were added.
- [ ] **CI threshold gate:** Often missing variance policy — verify rerun rules and guard bands are documented.
- [ ] **Comparator evidence:** Often missing version pinning — verify node/boa/quickjs/rustc versions are stored in artifact.
- [ ] **Performance win claim:** Often missing memory/GC impact — verify allocation + GC trend deltas are included.
- [ ] **Governance integration:** Often missing additive correctness gates — verify existing semantic gates remain unchanged or stronger.

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Apples-to-oranges benchmark contract | HIGH | Freeze optimization merges, normalize harness contract, regenerate baseline artifacts, invalidate prior comparisons |
| Noise-driven optimization decisions | MEDIUM | Re-run on controlled host with larger sample set, recompute thresholds using variance-aware policy |
| Semantic drift from fast paths | HIGH | Roll back optimization toggle, add guard/deopt fallback, backfill targeted semantic regressions, rerun full gates |
| Cache invalidation bugs | HIGH | Disable cache behind feature flag, implement shape/prototype versioning, add mutation stress suite before re-enable |
| Brittle/loose CI thresholds | MEDIUM | Split PR vs nightly policies, recalibrate guard bands from historical window, document threshold ownership |
| Baseline drift from version changes | MEDIUM | Pin versions, mark baseline-reset commit, publish before/after with old and new comparator stacks |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Apples-to-oranges benchmark contract | Phase 10 (PERF-01/02) | Harness spec + parity checks + dual mode (compile vs execute) report present |
| Benchmark noise mistaken for speedup | Phase 10, Phase 12 | Report includes variance metrics and environment metadata; rerun stability passes |
| Aggregate-only optimization bias | Phase 11, Phase 12 | Per-case non-regression gates + aggregate target both enforced |
| Semantic drift from fast paths | Phase 11 | Optimized hotspots have deopt tests and semantic edge regressions; test262-lite unchanged/stronger |
| Invalid cache assumptions | Phase 11 | Mutation-heavy regression pack and cache-disable parity checks pass |
| CPU win but GC loss | Phase 11, Phase 12 | Benchmark artifacts include allocation/GC deltas and long-run latency trend |
| Cross-layer optimization debt | Phase 11 (PERF-05) | PR architecture checklist approved; layering boundaries preserved |
| Brittle or loose thresholds | Phase 12 (TST-06) | Threshold policy, owner, guard bands, and rerun protocol are documented and enforced |
| Baseline drift from unpinned comparators | Phase 10, Phase 12 | Comparator versions pinned + baseline reset procedure tested |
| Perf evidence without semantic counter-evidence | Phase 12 (TST-05/TST-06) | Merge requires both performance artifacts and full semantic/governance gate success |

## Sources

- `.planning/PROJECT.md`
- `.planning/REQUIREMENTS.md`
- `crates/benchmarks/src/main.rs`
- `.github/workflows/ci.yml`
- `AGENTS.md`

---
*Pitfalls research for: qjs-rs v1.1 performance acceleration (semantics-stable runtime)*
*Researched: 2026-02-27*
