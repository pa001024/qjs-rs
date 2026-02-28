# Phase 11 Target Closure Evidence (Packet-B)

_Date:_ 2026-02-28  
_Plan:_ 11-03 (`11-hot-path-optimization-and-target-closure`)

## 1) Artifacts generated

- Baseline (locked reference): `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
- Packet-A candidate (existing): `target/benchmarks/engine-comparison.local-dev.packet-a.json`
- Packet-B candidate (this plan): `target/benchmarks/engine-comparison.local-dev.packet-b.json`
- Packet-B corroboration (ci-linux profile): `target/benchmarks/engine-comparison.ci-linux.packet-b.json`

Contract validation commands:

- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-b.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-b.json` ✅

## 2) PERF-04 packet deltas (local-dev)

### Aggregate (`qjs-rs`)

| Stage | qjs-rs mean ms | boa-engine mean ms | qjs/boa ratio |
|---|---:|---:|---:|
| Baseline | 1784.473 | 182.076 | 9.801x |
| Packet-A | 1294.700 | 201.816 | 6.415x |
| Packet-B | 1206.119 | 176.403 | 6.837x |

### Per-case (`qjs-rs` mean ms)

| Case | Baseline | Packet-A | Packet-B | Packet-A vs Baseline | Packet-B vs Packet-A |
|---|---:|---:|---:|---:|---:|
| `arith-loop` | 2153.732 | 1940.619 | 1844.848 | -9.90% | -4.94% |
| `fib-iterative` | 162.745 | 145.908 | 118.650 | -10.35% | -18.68% |
| `array-sum` | 4798.761 | 3071.256 | 2842.642 | -36.00% | -7.44% |
| `json-roundtrip` | 22.655 | 21.016 | 18.337 | -7.23% | -12.75% |

Packet-B hotspot attribution snapshot (`qjs_rs_hotspot_attribution.total`):

- `numeric_ops`: 31,417,400
- `identifier_resolution`: 136,257,800
- `array_indexed_property_get`: 2,801,400
- `array_indexed_property_set`: 2,800,000

## 3) PERF-03 closure gate result

Checker command:

```bash
python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-b.json \
  --require-qjs-lte-boa
```

Result: ❌ **FAILED**

- `aggregate.mean_ms_per_engine: require-qjs-lte-boa failed: candidate qjs-rs 1206.119211 > boa-engine 176.403164`

Interpretation:

- Packet-B improved Phase 11 hotspot families versus baseline and versus packet-A in local-dev measurements.
- Final locked PERF-03 closure criterion (`qjs-rs <= boa-engine`) is still unmet for this candidate.

## 4) PERF-05 maintainability boundary checklist

- [x] Runtime-core remains pure Rust (no C FFI added in VM/runtime core).
- [x] Packet-B logic is guarded with explicit fallback to canonical property semantics.
- [x] Dense-index fast path rejects non-dense arrays, accessor-backed indices, prototype-index hits, and exotic/proxy-like markers.
- [x] Changes remain layer-local (`crates/vm/src/*`, packet-specific tests, benchmark evidence/docs).

## 5) Quality/gate command outcomes (execution run)

- `cargo test -p vm packet_b_array_dense_index_fast_path_guarding -- --exact` ✅
- `cargo test -p vm perf_packet_b -- --nocapture` ✅
- `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-b.json --allow-missing-comparators` ✅
- `cargo run -p benchmarks --release -- --profile ci-linux --output target/benchmarks/engine-comparison.ci-linux.packet-b.json --allow-missing-comparators` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-b.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-b.json` ✅
- `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-b.json --require-qjs-lte-boa` ❌ (see Section 3)
- `cargo fmt --check` ❌ (workspace formatting drift outside packet-B scope)
- `cargo clippy --all-targets -- -D warnings` ❌ (pre-existing benchmark target lint debt outside packet-B scope)
- `cargo test` ❌ (Windows host memory/pagefile exhaustion during full workspace test build)

## 6) Closure status

- PERF-04 packet evidence: ✅ complete (two optimization packets with measurable deltas)
- PERF-05 maintainability boundary: ✅ complete
- PERF-03 closure (`qjs-rs <= boa-engine` on authoritative profile): ❌ not yet achieved in packet-B candidate

## 7) Plan 11-05 governance + packet-c closure rerun (2026-02-28)

### 7.1 Packet-c artifact refresh

- Regenerated artifact: `target/benchmarks/engine-comparison.local-dev.packet-c.json`
- Aggregate means (`local-dev`):
  - `qjs-rs`: `1678.421964 ms`
  - `boa-engine`: `189.600068 ms`
  - `qjs-rs/boa-engine`: `8.8524x`

### 7.2 Final command transcript (single sequence)

1. `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-c.json --allow-missing-comparators` ✅
2. `cargo fmt --check` ❌
3. `cargo clippy --all-targets -- -D warnings` ✅
4. `cargo test` ✅
5. `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-c.json` ✅
6. `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-c.json --require-qjs-lte-boa` ❌

### 7.3 Gate verdict

- Governance gate bundle (`fmt` + `clippy` + `test`): ❌ **RED** (`cargo fmt --check` failed)
- PERF-03 authoritative closure gate: ❌ **FAILED**
  - `candidate qjs-rs 1678.421964 > boa-engine 189.600068`

### 7.4 Blocker summary

1. Workspace formatting drift remains in VM files (outside this plan's ownership set), so the governance bundle is not fully green.
2. Packet-c aggregate performance still fails the locked PERF-03 requirement (`qjs-rs <= boa-engine`).

Closure remains **open**. No green governance-bundle verdict is recorded for this run.


