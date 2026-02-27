# Phase 10: Baseline Contract and Benchmark Normalization - Research

**Researched:** 2026-02-27  
**Domain:** Cross-engine benchmark contract, reproducibility metadata, artifact normalization  
**Confidence:** HIGH

## User Constraints

- Phase target is fixed: **Phase 10 - Baseline Contract and Benchmark Normalization**.
- This phase must satisfy **PERF-01** and **PERF-02** only.
- Roadmap success criteria require:
  1. JSON + human-readable benchmark artifacts for `qjs-rs`, `boa-engine`, `quickjs-c`, `nodejs`.
  2. Reproducibility metadata (engine versions, host info, run controls).
  3. Required hot-path case set: arithmetic loop, iterative calls, array build/sum, JSON roundtrip.
  4. Configurable sample/iteration controls for local + CI.
- Project boundary remains active: **runtime core stays pure Rust (no runtime-core C FFI)**.
- Project priority remains active: **semantic correctness > maintainability > performance** (AGENTS.md).
- Project discovery:
  - `CLAUDE.md`: not present.
  - `.agents/skills/`: not present.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PERF-01 | Reproducible benchmark outputs comparing `qjs-rs`, `boa-engine`, `quickjs-c`, and `nodejs` with machine-readable JSON + human-readable report artifacts | Define explicit benchmark contract (schema + run modes + metadata), normalize engine adapters, parameterize external paths/versions, and lock deterministic artifact layout + report generation flow |
| PERF-02 | Representative hot-path suite (arith loop, iterative function calls, array build/sum, JSON roundtrip) with configurable sample/iteration controls | Keep required 4-case suite as contract-managed catalog, enforce configurability in CLI + docs + artifact metadata, and add contract tests for case coverage + controls |
</phase_requirements>

## Summary

Phase 10 is a **measurement-contract phase**, not an optimization phase. The repository already has a usable benchmark pipeline (`crates/benchmarks` + JSON output + markdown/SVG report), but it is not yet strong enough to be the optimization decision baseline for Phase 11. The missing piece is a formalized contract that makes runs comparable across engines and reproducible across environments.

The biggest planning risk is hidden comparability drift: current adapters do not execute identical timing semantics (notably `qjs-rs` compiles once before timing loop while other engines `eval` inside timing loop), and result-parity checks are not normalized across engines. Without fixing this first, Phase 11 perf decisions can be directionally wrong.

**Primary recommendation:** plan Phase 10 around a contract-first deliverable sequence: (1) benchmark schema/version + case catalog + run-mode policy, (2) adapter normalization + metadata completeness + deterministic artifact paths, (3) contract tests + docs/runbook updates proving PERF-01/PERF-02 closure.

## Current Baseline Reality (What Already Exists vs What Is Missing)

| Area | Current State | Gap to Close in Phase 10 |
|------|---------------|--------------------------|
| Required case portfolio | All 4 required cases already exist in `crates/benchmarks/src/main.rs` (`arith-loop`, `fib-iterative`, `array-sum`, `json-roundtrip`) | Convert from implicit code list to explicit contract catalog (stable IDs + workload-family labels + expected output/checksum policy) |
| Configurable controls | `--iterations`, `--samples`, `--output` already implemented | Add run-profile policy (`local-dev`, `ci-linux`) + persist full control set in artifact metadata |
| Machine-readable artifact | JSON report exists at `target/benchmarks/engine-comparison.json` | Add schema version and stronger reproducibility fields (run mode, comparator paths/versions, optional git commit, timing mode, warmup policy) |
| Human-readable artifact | `scripts/render_engine_benchmark_report.py` generates markdown + SVG | Ensure report renders contract metadata explicitly and flags incomplete/unsupported adapters |
| Cross-engine comparators | `qjs-rs`, `boa-engine`, `nodejs`, `quickjs-c` adapters exist | Normalize adapter semantics and preflight requirements; remove hard-coded path assumptions as hidden machine dependency |
| CI integration | No performance benchmark normalization gate in current CI | Add non-flaky contract checks (schema + case coverage + metadata completeness) while leaving strict perf thresholds to later phases |

## Standard Stack

### Core

| Component | Version/Source | Purpose | Why Standard for This Phase |
|---|---|---|---|
| `crates/benchmarks` (Rust binary) | in-repo | Single runner for all engines and JSON artifact production | Already integrated; lowest-risk extension point for PERF-01/PERF-02 |
| `serde` + `serde_json` | in `crates/benchmarks/Cargo.toml` | Stable, machine-parseable benchmark report serialization | Existing stack; supports schema evolution with explicit version fields |
| `scripts/render_engine_benchmark_report.py` | in-repo | Transform JSON into markdown + chart artifacts | Keeps human-readable evidence generation deterministic and reviewable |
| `docs/engine-benchmarks.md` | in-repo | Canonical runbook (how benchmark contract is executed) | Existing documentation anchor; should become contract source for local/CI execution |

### Supporting

| Component | Purpose | When to Use |
|---|---|---|
| `.planning/REQUIREMENTS.md` + `.planning/ROADMAP.md` | Contract-level acceptance truth for PERF-01/PERF-02 | During planning and verification mapping |
| `docs/reports/engine-benchmark-report.md` | Baseline artifact shape reference | While defining normalized schema/report expectations |
| GitHub Actions (`.github/workflows/ci.yml`) | Future execution location for contract checks | Add only contract-validity checks in this phase (not strict perf pass/fail thresholds yet) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| Extending current benchmark runner | Rebuild benchmark system from scratch | High rewrite risk; loses existing working artifacts and slows Phase 10 |
| Contract tests in repo | Manual checklist-only validation | Faster initially but not reproducible/auditable for PERF-01 |
| Single aggregate-only view | Per-case + aggregate contract outputs | Aggregate-only hides case regressions and weakens optimization guidance |

## Architecture Patterns

### Pattern 1: Contract-First Benchmark Schema

**What:** Introduce an explicit schema version and required metadata fields that are validated in tests.

**When to use:** Always; this is the foundation for reproducibility and comparability.

**Recommended minimum fields:**
- `schema_version`
- `generated_at_utc`
- `config` (`samples`, `iterations`, warmup count, timing mode)
- `environment` (os, arch, cpu count, rustc, node version, quickjs version, boa version)
- `cases[*]` (stable id/title/description/family)
- `cases[*].engines[*]` timing summary + checksum/result-normalization payload
- `aggregate` with documented derivation

### Pattern 2: Adapter Parity Contract

**What:** Every engine adapter must declare and follow the same timing contract mode for a run.

**When to use:** `qjs-rs`, `boa-engine`, `nodejs`, `quickjs-c` execution paths.

**Recommendation:** Explicitly support run mode(s) in artifact metadata (for example `eval_per_iteration` and/or `compile_once_execute_many`) and ensure all engines in a run use the same mode.

### Pattern 3: Deterministic Artifact Layout

**What:** Standardize output locations and naming policy independent of developer machine quirks.

**When to use:** Every benchmark run that is used as evidence.

**Recommendation:** keep default deterministic paths and encode profile in filename (example: `target/benchmarks/engine-comparison.ci-linux.json`) plus generated report pair.

### Pattern 4: Preflight + Fail-Fast Comparator Validation

**What:** Validate comparator availability/versions before long benchmark runs.

**When to use:** Start of benchmark command.

**Recommendation:** surface actionable diagnostics for missing Node/QuickJS path/version mismatch rather than partial silent behavior.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Report data contract | Markdown parsing as source of truth | Versioned JSON schema validated in tests | Markdown is presentation; JSON is machine contract |
| Comparator discovery | Hard-coded machine paths only | Configurable adapter paths + preflight checks | Hard-coded paths break reproducibility on other hosts/CI |
| Benchmark validity proof | Manual spot-checks | Automated contract tests for case IDs + metadata + schema | Required for repeatable PERF-01 closure |
| Cross-engine equivalence | Implicit assumptions | Explicit timing-mode field + adapter parity assertions | Prevents apples-to-oranges optimization decisions |

## Common Pitfalls (Phase-10 Relevant)

### Pitfall 1: Cross-engine timing-mode mismatch
- **Current signal:** `qjs-rs` compiles once before loop; other engines call `eval` in timed loop.
- **Risk:** invalid comparability; optimization priorities become unreliable.
- **Avoidance:** encode timing mode in contract and enforce adapter parity.

### Pitfall 2: Incomplete result parity enforcement
- **Current signal:** checksum/result handling differs by adapter (`boa` path currently increments a counter instead of normalizing returned value).
- **Risk:** semantic mismatches can hide behind benchmark numbers.
- **Avoidance:** normalize result extraction per case and persist/validate checksums across engines.

### Pitfall 3: Hidden machine dependencies
- **Current signal:** QuickJS invocation path is hard-coded to `/mnt/d/dev/QuickJS` via WSL.
- **Risk:** benchmark pipeline non-portable; reproducibility claim weak.
- **Avoidance:** parameterized comparator paths + explicit preflight failure messages + documented setup contract.

### Pitfall 4: Coarse/ambiguous timing precision across adapters
- **Current signal:** adapter timing mechanisms differ (e.g., Node uses `hrtime`, QuickJS path uses `Date.now()`-based script timing).
- **Risk:** skewed variance and unfair per-case comparisons.
- **Avoidance:** document timing source per adapter and normalize policy (or increase workload to reduce quantization impact) with metadata transparency.

### Pitfall 5: Artifact schema drift
- **Current signal:** report JSON has no explicit schema version today.
- **Risk:** downstream scripts silently break after shape changes.
- **Avoidance:** add `schema_version` and schema conformance tests.

## Code Examples (Planning Targets)

### Example 1: Contracted report envelope (shape)

```json
{
  "schema_version": "bench.v1",
  "generated_at_utc": "2026-02-27T13:55:32.421Z",
  "run_profile": "local-dev",
  "timing_mode": "eval_per_iteration",
  "config": {
    "samples": 7,
    "iterations": 200,
    "warmup_iterations": 3
  }
}
```

### Example 2: Comparator preflight contract (pseudocode)

```rust
for engine in required_engines {
    let info = adapter.preflight()?; // version + availability + command path
    metadata.engines.insert(engine.id(), info);
}
```

### Example 3: Case-catalog contract test idea

```text
assert required_case_ids == {arith-loop, fib-iterative, array-sum, json-roundtrip}
assert samples > 0 && iterations > 0
assert schema_version present
```

## Phase Planning Implications (Recommended Plan Slices)

1. **10-01 Contract Spec + Schema Lock**
   - Define benchmark contract doc (run modes, required metadata, case catalog, artifact paths).
   - Add schema version to JSON.
   - Add tests that fail on schema drift and missing required cases.

2. **10-02 Adapter Normalization + Reproducibility Hardening**
   - Normalize adapter timing policy and checksum/result parity handling.
   - Parameterize Node/QuickJS adapter paths and add preflight diagnostics.
   - Persist comparator versions/paths/run mode in artifacts.

3. **10-03 Reporting + Documentation + Verification Closure**
   - Update report renderer to surface contract metadata.
   - Update `docs/engine-benchmarks.md` with profile-based commands and reproducibility checklist.
   - Produce baseline artifacts under deterministic paths and capture verification evidence for PERF-01/PERF-02.

## Open Questions (Resolve During Planning)

1. **Which timing mode is the Phase 10 default contract?**
   - `eval_per_iteration` only, or dual-mode reporting (`eval` + `execute`) where supported.
2. **How strict should comparator availability be for local runs?**
   - Hard fail if one comparator missing vs allow explicit opt-out profiles.
3. **What minimum metadata is mandatory for claiming reproducibility?**
   - Decide final required fields now to avoid rework in Phase 11 artifacts.
4. **Where should pinned comparator policy live?**
   - Benchmark docs only vs dedicated baseline policy file for later CI gating.

## Sources

Primary files read:
- `C:/Users/Administrator/.codex/agents/gsd-phase-researcher.md`
- `AGENTS.md`
- `.planning/REQUIREMENTS.md`
- `.planning/STATE.md`
- `.planning/research/SUMMARY.md`
- `.planning/research/STACK.md`
- `.planning/research/FEATURES.md`
- `.planning/research/ARCHITECTURE.md`
- `.planning/research/PITFALLS.md`
- `.planning/ROADMAP.md`

Implementation baseline references:
- `.planning/PROJECT.md`
- `docs/engine-benchmarks.md`
- `docs/reports/engine-benchmark-report.md`
- `target/benchmarks/engine-comparison.json`
- `crates/benchmarks/src/main.rs`
- `scripts/render_engine_benchmark_report.py`
- `crates/benchmarks/Cargo.toml`
- `.github/workflows/ci.yml`
- `.planning/config.json`

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** (already implemented in-repo; extension path is clear)
- Architecture patterns: **HIGH** (directly grounded in current harness + roadmap criteria)
- Pitfalls/gaps: **HIGH** (confirmed from current code and artifacts)

**Research date:** 2026-02-27  
**Valid until:** 2026-03-20
