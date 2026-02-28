# Phase 11 Packet-C Evidence (Identifier/Global Lookup Candidate)

_Date:_ 2026-02-28  
_Plan:_ 11-04 (`11-hot-path-optimization-and-target-closure`)

## 1) Artifacts generated

- Baseline (locked reference): `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
- Packet-B candidate (previous): `target/benchmarks/engine-comparison.local-dev.packet-b.json`
- Packet-C candidate (this plan): `target/benchmarks/engine-comparison.local-dev.packet-c.json`
- Packet-C corroboration (ci-linux profile): `target/benchmarks/engine-comparison.ci-linux.packet-c.json`

Contract validation commands:

- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-c.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-c.json` ✅

## 2) Packet-C benchmark deltas (local-dev)

### Aggregate (`qjs-rs`)

| Stage | qjs-rs mean ms | boa-engine mean ms | qjs/boa ratio |
|---|---:|---:|---:|
| Baseline | 1784.473 | 182.076 | 9.801x |
| Packet-B | 1206.119 | 176.403 | 6.837x |
| Packet-C | 1666.496 | 189.938 | 8.774x |

Aggregate qjs-rs deltas:

- Packet-C vs baseline: **-6.61%**
- Packet-C vs packet-B: **+38.17%** (regression)

### Per-case (`qjs-rs` mean ms)

| Case | Baseline | Packet-B | Packet-C | Packet-C vs Baseline | Packet-C vs Packet-B |
|---|---:|---:|---:|---:|---:|
| `arith-loop` | 2153.732 | 1844.848 | 2574.104 | +19.52% | +39.53% |
| `fib-iterative` | 162.745 | 118.650 | 190.318 | +16.94% | +60.40% |
| `array-sum` | 4798.761 | 2842.642 | 3873.349 | -19.28% | +36.26% |
| `json-roundtrip` | 22.655 | 18.337 | 28.215 | +24.54% | +53.87% |

Packet-C hotspot attribution snapshot (`qjs_rs_hotspot_attribution.total`):

- `numeric_ops`: 31,417,400
- `identifier_resolution`: 136,257,800
- `array_indexed_property_get`: 2,801,400
- `array_indexed_property_set`: 2,800,000

## 3) PERF-03 closure gate result

Checker command:

```bash
python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-c.json \
  --require-qjs-lte-boa
```

Result: ❌ **FAILED**

- `aggregate.mean_ms_per_engine: require-qjs-lte-boa failed: candidate qjs-rs 1666.496393 > boa-engine 189.938318`

Interpretation:

- Packet-C preserves semantic parity and emits expected guard activity in targeted VM tests.
- Under authoritative benchmark policy, packet-C does **not** close PERF-03 and regresses versus packet-B aggregate performance.

## 4) Quality/gate command outcomes (execution run)

- `cargo test -p vm packet_c_identifier_resolution_guarding -- --exact` ✅
- `cargo test -p vm perf_packet_c -- --nocapture` ✅
- `cargo test -p benchmarks packet_c_output_path_enables_packet_c_runtime_toggle -- --exact` ✅
- `cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-c.json --allow-missing-comparators` ✅
- `cargo run -p benchmarks --release -- --profile ci-linux --output target/benchmarks/engine-comparison.ci-linux.packet-c.json --allow-missing-comparators` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-c.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-c.json` ✅
- `python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-c.json --require-qjs-lte-boa` ❌

## 5) Closure status

- Packet-C VM implementation + parity coverage: ✅ complete
- Packet-C contract-valid artifacts + checker transcript: ✅ complete
- PERF-03 closure (`qjs-rs <= boa-engine` on authoritative profile): ❌ still open
