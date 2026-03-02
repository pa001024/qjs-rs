---
phase: 11-hot-path-optimization-and-target-closure
phase_number: "11"
verified: 2026-03-02T07:32:15.800Z
status: gaps_found
score: "evidence synchronized through 11-09; PERF-03 quickjs-ratio gate still red"
requirements_checked:
  - PERF-03
  - PERF-04
  - PERF-05
---

# Phase 11 Verification Report

## Goal Verdict

Phase 11 goal is **not achieved yet**.

Reason: the hard closure target (`PERF-03`: aggregate `qjs-rs <= 1.25x quickjs-c` on locked profile) is still red in the latest authoritative packet-e run (`qjs-rs/quickjs-c = 6.136312x`).

## Inputs Audited

- Plans/Summaries: `11-01..11-09` PLAN + SUMMARY set
- Evidence bundles:
  - `11-PACKET-A-EVIDENCE.md`
  - `11-PACKET-C-EVIDENCE.md`
  - `11-PACKET-D-EVIDENCE.md`
  - `11-TARGET-CLOSURE-EVIDENCE.md`
- Traceability/governance sources:
  - `.planning/ROADMAP.md`
  - `.planning/REQUIREMENTS.md`
  - `.planning/STATE.md`
  - `AGENTS.md`
- Authoritative machine-readable run artifacts:
  - `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json` (`generated_at_utc`: `2026-03-02T07:30:27.870Z`)
  - `target/benchmarks/engine-comparison.local-dev.packet-e.json` (`generated_at_utc`: `2026-03-02T07:32:15.800Z`)

## Authoritative 11-09 Candidate Results (single provenance source)

Ordered command outcomes for the authoritative 11-09 run set:

1. `cargo fmt --check`: ❌
2. `cargo clippy -p vm -p benchmarks -- -D warnings`: ✅
3. `cargo test -p vm perf_packet_d -- --nocapture`: ✅
4. `cargo test -p vm perf_hotspot_attribution -- --nocapture`: ✅
5. `cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators`: ✅
6. `cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-e.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators`: ✅
7. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`: ✅
8. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-e.json`: ✅
9. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-e.json --require-qjs-lte-quickjs-ratio 1.25`: ❌

Packet-e candidate hash provenance:

- `path`: `target/benchmarks/engine-comparison.local-dev.packet-e.json`
- `hash/sha256`: `e2c83552ed5f89129b700885c8ec67476d26214fb96ec0fad94223723d465a9c`

Aggregate means (11-09 packet-e candidate):

- `qjs-rs`: `98.181000`
- `quickjs-c`: `16.000000`
- `qjs-rs/quickjs-c`: `6.136312x`

## Must-Have Truth Audit (11-01..11-09)

| Plan | Must-have truths | Result | Notes |
|---|---:|---|---|
| 11-01 | 3 | 3/3 ✅ | Closure policy + checker, perf metadata/hotspot attribution contract, attribution toggle/parity are present and test-covered. |
| 11-02 | 3 | 3/3 ✅ | Packet-A guarded numeric/binding fast paths + fallback parity + contract-valid packet evidence are present. |
| 11-03 | 3 | 1/3 ⚠️ | Packet-B implementation/parity evidence is present, but PERF-03 proof and all-green governance expectation are not met. |
| 11-04 | 3 | 2/3 ⚠️ | Packet-C implementation/parity and before/after reporting are present; historical legacy closure pass (`--require-qjs-lte-boa`) was not met. |
| 11-05 | 5 | 4/5 ⚠️ | Gap-closure sync + packet stability + failure-path doc synchronization are present; governance/perf closure remained open. |
| 11-06 | 3 | 2/3 ⚠️ | Packet-D implementation and parity guard evidence are present; PERF-03 closure remained open. |
| 11-07 | 2 | 1/2 ⚠️ | Authoritative bundle provenance and governance gates are green, but PERF-03 remained red so joint closure condition was unmet. |
| 11-08 | 3 | 3/3 ✅ | PERF-03 checker/policy/traceability alignment to active quickjs-ratio gate is complete and self-tested. |
| 11-09 | 3 | 2/3 ⚠️ | Guarded identifier-call dispatch optimization + parity/hotspot evidence landed; authoritative quickjs-ratio checker is still red. |

Net: **22/27 truths verified**.

## Requirement Cross-Reference (Plan Frontmatter ↔ Traceability)

All nine Phase 11 plans (`11-01..11-09`) declare the same requirement set in frontmatter:
- `PERF-03`
- `PERF-04`
- `PERF-05`

Traceability status in `.planning/REQUIREMENTS.md` currently remains:
- `PERF-03`: **Open**
- `PERF-04`: **Open**
- `PERF-05`: **Open**

Verification conclusion per requirement:

| Requirement | Verification result | Evidence summary |
|---|---|---|
| PERF-03 | ❌ Unsatisfied | Latest authoritative 11-09 packet-e checker run failed: `qjs-rs/quickjs-c 6.136312 > 1.25` (`qjs-rs=98.181000`, `quickjs-c=16.000000`). |
| PERF-04 | ⚠️ Implemented evidence exists, closure-state open | Multiple hot-path packets (A/B/C/D) and before/after evidence exist, but phase closure remains gated by unresolved PERF-03. |
| PERF-05 | ⚠️ Boundary evidence positive, closure-state open | No runtime-core C FFI introduced; guarded fallback patterns and layer-local changes are present; milestone traceability remains open until PERF-03 is satisfied in authoritative bundle checks. |

## Governance/Boundary Checks

- Pure-Rust runtime-core boundary: no C FFI indicators found in `crates/vm`, `crates/runtime`, `crates/bytecode`, `crates/builtins` scan.
- Authoritative 11-09 governance/test outcomes (from command outputs used for packet-e evidence):
  - `fmt`: ❌
  - `clippy`: ✅
  - `test (targeted packet suites)`: ✅

## Final Status

- **status:** `gaps_found`
- **Phase 11 closure:** **OPEN**

### Top remaining blockers

1. **PERF-03 target not met**: latest authoritative packet-e ratio is `6.136312x`, above `1.25x` closure threshold.
2. **Governance bundle not fully green in 11-09 run set**: `cargo fmt --check` remains red, so joint closure evidence is still incomplete.
