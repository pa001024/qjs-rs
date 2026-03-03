---
phase: 11-hot-path-optimization-and-target-closure
phase_number: "11"
verified: 2026-03-03T06:59:46.073971Z
status: gaps_found
score: "latest authoritative 11-14 packet-h evidence remains below PERF-03 target (6.260034x > 1.25x); governance transcript is red on fmt while clippy/targeted tests stay green"
requirements_checked:
  - PERF-03
  - PERF-04
  - PERF-05
---

# Phase 11 Verification Report

## Goal Verdict

Phase 11 goal is **not achieved yet**.

Reason: the hard closure target (`PERF-03`: aggregate `qjs-rs <= 1.25x quickjs-c` on locked profile) is still red in the latest authoritative packet-h run (`qjs-rs/quickjs-c = 6.260034x`).

## Inputs Audited

- Plans/Summaries: `11-01..11-14` PLAN + SUMMARY set
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
  - `target/benchmarks/phase11-closure-bundle.packet-h.json`
  - `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json` (`generated_at_utc`: `2026-03-02T07:30:27.870Z`)
  - `target/benchmarks/engine-comparison.local-dev.packet-h.json` (`generated_at_utc`: `2026-03-03T06:51:55.493Z`)
  - `target/benchmarks/perf-target.packet-h.verdict.json`

## Authoritative 11-14 Candidate Results (single provenance source)

Ordered command outcomes for the authoritative 11-14 run set (from `target/benchmarks/phase11-closure-bundle.packet-h.json`):

1. `cargo fmt --check`: ❌ (`exit_code=1`, transcript: `target/benchmarks/fmt.packet-h.stderr.log`)
2. `cargo clippy -p vm -p benchmarks -- -D warnings`: ✅
3. `cargo test -p vm perf_packet_d -- --nocapture`: ✅
4. `cargo test -p vm perf_hotspot_attribution -- --nocapture`: ✅
5. `cargo run -p benchmarks --bin benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-h.json --quickjs-path scripts/quickjs-wsl.cmd --strict-comparators`: ✅
6. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-h.json`: ✅
7. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-h.json --require-qjs-lte-quickjs-ratio 1.25`: ❌ (`qjs-rs/quickjs-c 6.260034 > 1.25`)

Packet-h candidate hash provenance:

- `path`: `target/benchmarks/engine-comparison.local-dev.packet-h.json`
- `hash/sha256`: `91a2559fdf1264f7bb1fb29f8cabde4733a277a8ca4ab848c9a96257bf251e94`

Aggregate means (11-14 packet-h candidate):

- `qjs-rs`: `81.827593`
- `quickjs-c`: `13.071429`
- `qjs-rs/quickjs-c`: `6.260034x`

Checker verdict/log capture (same candidate path):

1. `target/benchmarks/perf-target.packet-h.verdict.json`: `status=threshold_fail_expected`, `exit_code=1`
2. `target/benchmarks/perf-target.packet-h.stderr.log`: `require-qjs-lte-quickjs-ratio failed: ... 6.260034 > 1.250000`

## Must-Have Truth Audit (11-01..11-14)

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
| 11-10 | 3 | 2/3 ⚠️ | Governance was restored to green and packet-f candidate evidence is contract-valid; PERF-03 quickjs-ratio checker remains red (`6.085281x`). |
| 11-11 | 3 | 2/3 ⚠️ | Final guarded optimization + packet-final evidence landed with green governance, but PERF-03 quickjs-ratio checker remains red (`5.755257x`). |
| 11-12 | 3 | 3/3 ✅ | Packet-g guarded identifier fallback path, contract-valid packet-g artifact, and synchronized closure wording are present; ratio gate remains red and is explicitly documented. |
| 11-13 | 3 | 3/3 ✅ | Packet-h lexical-slot guard path, parity/hotspot coverage, and packet-h contract wiring landed with strict-comparator smoke evidence. |
| 11-14 | 3 | 3/3 ✅ | Authoritative packet-h candidate + machine-checkable closure bundle are present, and PERF-05 boundary refresh is recorded in the same packet-h cycle. PERF-03 remains red. |

Net: **34/42 truths verified**.

## Requirement Cross-Reference (Plan Frontmatter ↔ Traceability)

All active Phase 11 plans (`11-01..11-14`) declare the same requirement set in frontmatter:
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
| PERF-03 | ❌ Unsatisfied | Latest authoritative 11-14 packet-h checker run failed: `qjs-rs/quickjs-c 6.260034 > 1.25` (`qjs-rs=81.827593`, `quickjs-c=13.071429`). |
| PERF-04 | ⚠️ Implemented evidence exists, closure-state open | Multiple hot-path packets (A/B/C/D/E/F/final/g/h) and before/after evidence exist, but phase closure remains gated by unresolved PERF-03. |
| PERF-05 | ⚠️ Boundary evidence positive, closure-state open | `target/benchmarks/perf05-boundary-scan.packet-h.log` records a clean runtime-core scan (`extern "C"`/`unsafe` not found); phase closure remains blocked only by PERF-03. |

## Governance/Boundary Checks

- Authoritative 11-14 governance/test outcomes:
  - `fmt`: ❌ (`exit_code=1`)
  - `clippy`: ✅
  - `test (targeted packet suites)`: ✅
- Pure-Rust runtime-core boundary scan:
  - `rg --line-number 'extern\\s+"C"|\\bunsafe\\b' crates/vm crates/runtime crates/bytecode crates/builtins`
  - Result: no matches (`target/benchmarks/perf05-boundary-scan.packet-h.log`, clean boundary signal).

## Final Status

- **status:** `gaps_found`
- **Phase 11 closure:** **OPEN**

### Top remaining blockers

1. **PERF-03 target not met**: latest authoritative packet-h ratio is `6.260034x`, above `1.25x` closure threshold.
