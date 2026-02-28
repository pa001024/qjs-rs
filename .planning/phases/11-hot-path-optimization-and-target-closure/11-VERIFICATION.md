---
phase: 11-hot-path-optimization-and-target-closure
phase_number: "11"
verified: 2026-02-28T07:54:08Z
status: gaps_found
score: 7/9 must-have truths verified (goal gate not met)
requirements_checked:
  - PERF-03
  - PERF-04
  - PERF-05
---

# Phase 11 Verification Report

## Goal Verdict

Phase 11 goal is **not fully achieved**.

- Achieved: evidence-backed hot-path optimization packets and maintainability-safe guarded fallbacks.
- Not achieved: authoritative PERF-03 closure gate (`qjs-rs <= boa-engine` aggregate mean under locked profile) still fails.

Therefore Phase 11 verification status is `gaps_found`.

## Scope

Validated requested planning/evidence inputs:

- `11-01-PLAN.md`, `11-02-PLAN.md`, `11-03-PLAN.md`
- `11-01-SUMMARY.md`, `11-02-SUMMARY.md`, `11-03-SUMMARY.md`
- `11-PACKET-A-EVIDENCE.md`, `11-TARGET-CLOSURE-EVIDENCE.md`
- `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md`, `AGENTS.md`

Validated implementation/evidence artifacts in repo:

- `.github/scripts/check_perf_target.py`
- `docs/performance-closure-policy.md`
- `crates/benchmarks/src/contract.rs`
- `crates/benchmarks/src/main.rs`
- `crates/benchmarks/tests/hot_path_contract.rs`
- `crates/benchmarks/tests/perf_packet_a_report.rs`
- `crates/vm/src/perf.rs`
- `crates/vm/src/fast_path.rs`
- `crates/vm/src/lib.rs`
- `crates/vm/tests/perf_hotspot_attribution.rs`
- `crates/vm/tests/perf_packet_a.rs`
- `crates/vm/tests/perf_packet_b.rs`
- `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
- `target/benchmarks/engine-comparison.local-dev.packet-a.json`
- `target/benchmarks/engine-comparison.local-dev.packet-b.json`

## Must-Have Audit

### Plan 11-01 (policy + attribution foundation)

**Status: PASS**

- Closure policy is explicit and checker-enforced:
  - `.github/scripts/check_perf_target.py:15-17, 431-438, 716-718`
  - `docs/performance-closure-policy.md:9-20, 75-79`
- Benchmark contract includes machine-checkable perf-target metadata and optional hotspot attribution:
  - `crates/benchmarks/src/contract.rs:123-135, 609-611`
  - `crates/benchmarks/src/main.rs:622-638, 932-949, 967-968`
- VM hotspot attribution is opt-in and semantics-preserving under toggle tests:
  - `crates/vm/src/perf.rs:30-52, 54-89`
  - `crates/vm/tests/perf_hotspot_attribution.rs:61-73`

### Plan 11-02 (packet-A numeric/binding)

**Status: PASS**

- Arithmetic/call-heavy hot paths are optimized with measurable before/after evidence:
  - `crates/vm/src/fast_path.rs:8-31`
  - `crates/vm/src/lib.rs:21333-21356, 21416-21434`
  - `11-PACKET-A-EVIDENCE.md:36-47`
- Numeric/binding fast paths preserve fallback safety:
  - `crates/vm/src/lib.rs:21344-21356, 14550-14579, 15544-15623`
  - `crates/vm/tests/perf_packet_a.rs:33-51, 75-93, 115-123`
- Packet-A artifacts are contract-valid and baseline-comparable:
  - `11-PACKET-A-EVIDENCE.md:30-33, 51-57`
  - `crates/benchmarks/tests/perf_packet_a_report.rs:19-32`

### Plan 11-03 (packet-B + target closure)

**Status: GAPS FOUND**

- ✅ Second guarded optimization packet for array/property-heavy paths exists with semantic parity coverage:
  - `crates/vm/src/lib.rs:14920-15002, 15010-15016`
  - `crates/vm/tests/perf_packet_b.rs:61-67, 71-88, 91-121, 124-142, 145-189`
- ❌ Final candidate does **not** satisfy PERF-03 closure gate (`qjs-rs <= boa-engine`):
  - `11-TARGET-CLOSURE-EVIDENCE.md:55-63, 84-89`
  - Runtime recheck: `python .github/scripts/check_perf_target.py --baseline ...phase11-baseline.json --candidate ...packet-b.json --require-qjs-lte-boa` failed with `candidate qjs-rs 1206.119211 > boa-engine 176.403164`.
- ❌ Governance gates are not all passing in current workspace state (required by 11-03 must-have truth):
  - `11-TARGET-CLOSURE-EVIDENCE.md:79-82`
  - Runtime recheck: `cargo fmt --check` failed (formatting drift in benchmarks/vm files).

## Summary/Evidence Consistency Check

- `11-03-SUMMARY.md` correctly states PERF-03 remains unresolved (`11-03-SUMMARY.md:42, 98-99, 107-109`).
- `ROADMAP.md` also records this nuance (`.planning/ROADMAP.md:15`).
- However, summary frontmatter and `REQUIREMENTS.md` traceability still mark PERF-03/PERF-04/PERF-05 as completed (`11-03-SUMMARY.md:32-35`, `.planning/REQUIREMENTS.md:46-48`), so PERF-03 should be treated as **accounted but not closed** until the checker gate passes.

## Requirement ID Cross-Reference (Machine-Parseable)

```json
{
  "plan_frontmatter_requirements": {
    "11-01-PLAN.md": ["PERF-03", "PERF-04", "PERF-05"],
    "11-02-PLAN.md": ["PERF-03", "PERF-04", "PERF-05"],
    "11-03-PLAN.md": ["PERF-03", "PERF-04", "PERF-05"]
  },
  "unique_requirement_ids": ["PERF-03", "PERF-04", "PERF-05"],
  "requirements_md_accounting": {
    "PERF-03": {
      "defined": true,
      "traceability_phase": "Phase 11",
      "traceability_status": "Completed",
      "verification_evidence_status": "open_gate_not_met"
    },
    "PERF-04": {
      "defined": true,
      "traceability_phase": "Phase 11",
      "traceability_status": "Completed",
      "verification_evidence_status": "met"
    },
    "PERF-05": {
      "defined": true,
      "traceability_phase": "Phase 11",
      "traceability_status": "Completed",
      "verification_evidence_status": "met_with_governance_gap_in_plan_11_03_must_have"
    }
  },
  "unknown_requirement_ids": [],
  "unaccounted_requirement_ids": []
}
```

## Command Evidence (Verification Run)

Executed during this verification:

- `python .github/scripts/check_perf_target.py --self-test` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-b.json` ✅
- `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-b.json --require-qjs-lte-boa` ❌
- `cargo test -p vm perf_hotspot_attribution_records_opcode_families -- --exact` ✅
- `cargo test -p vm packet_a_numeric_fast_path_parity -- --exact` ✅
- `cargo test -p vm packet_b_array_dense_index_fast_path_guarding -- --exact` ✅
- `cargo test -p benchmarks hot_path_contract_serializes_perf_target_and_hotspot_attribution -- --exact` ✅
- `cargo test -p benchmarks perf_packet_a_report_tags_packet_metadata_from_output_path -- --exact` ✅
- `cargo fmt --check` ❌

## Final Status

- `status: gaps_found`
- Phase 11 requirement IDs are all accounted for (`PERF-03`, `PERF-04`, `PERF-05`).
- Remaining blockers for full goal achievement:
  1. PERF-03 aggregate closure gate failure (`qjs-rs` still slower than `boa-engine` under authoritative policy).
  2. 11-03 must-have expectation that governance gates pass is not currently satisfied.
