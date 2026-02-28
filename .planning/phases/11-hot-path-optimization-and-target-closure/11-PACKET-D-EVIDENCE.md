# Phase 11 Packet-D Evidence (Identifier-Slot Cache Candidate)

_Date:_ 2026-02-28  
_Plan:_ 11-06 (`11-hot-path-optimization-and-target-closure`)

## 1) Artifacts generated

- Baseline (locked reference): `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`
- Packet-B candidate (previous): `target/benchmarks/engine-comparison.local-dev.packet-b.json`
- Packet-C candidate (previous): `target/benchmarks/engine-comparison.local-dev.packet-c.json`
- Packet-D candidate (this plan): `target/benchmarks/engine-comparison.local-dev.packet-d.json`
- Packet-D corroboration (`ci-linux`): `target/benchmarks/engine-comparison.ci-linux.packet-d.json`

Contract validation commands:

- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-d.json` ✅
- `python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-d.json` ✅

Packet-D benchmark metadata confirms packet tagging + runtime toggle wiring:

- `perf_target.optimization_tag = "packet-d"`
- `perf_target.packet_id = "packet-d"`
- benchmark log includes `packet_d_enabled=true`

## 2) Packet-D benchmark deltas (`local-dev`)

### Aggregate (`qjs-rs`)

| Stage | qjs-rs mean ms | boa-engine mean ms | qjs/boa ratio |
|---|---:|---:|---:|
| Baseline | 1784.473 | 182.076 | 9.801x |
| Packet-B | 1206.119 | 176.403 | 6.837x |
| Packet-C | 1666.678 | 193.375 | 8.619x |
| Packet-D | 1383.310 | 176.069 | 7.857x |

Aggregate qjs-rs deltas:

- Packet-D vs baseline: **-22.48%**
- Packet-D vs packet-B: **+14.69%** (regression vs packet-B)
- Packet-D vs packet-C: **-17.00%**

### Per-case (`qjs-rs` mean ms)

| Case | Baseline | Packet-B | Packet-C | Packet-D | Packet-D vs Baseline | Packet-D vs Packet-B | Packet-D vs Packet-C |
|---|---:|---:|---:|---:|---:|---:|---:|
| `arith-loop` | 2153.732 | 1844.848 | 2613.968 | 2094.709 | -2.74% | +13.54% | -19.86% |
| `fib-iterative` | 162.745 | 118.650 | 187.458 | 161.485 | -0.77% | +36.10% | -13.86% |
| `array-sum` | 4798.761 | 2842.642 | 3843.370 | 3255.826 | -32.15% | +14.54% | -15.29% |
| `json-roundtrip` | 22.655 | 18.337 | 21.917 | 21.219 | -6.34% | +15.72% | -3.18% |

Packet-D hotspot attribution snapshot (`qjs_rs_hotspot_attribution.total`):

- `numeric_ops`: 31,417,400
- `identifier_resolution`: 136,257,800
- `array_indexed_property_get`: 2,801,400
- `array_indexed_property_set`: 2,800,000

## 3) PERF-03 closure gate result

Checker command:

```bash
python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-d.json \
  --require-qjs-lte-boa
```

Result: ❌ **FAILED**

- `aggregate.mean_ms_per_engine: require-qjs-lte-boa failed: candidate qjs-rs 1383.310014 > boa-engine 176.068693`

Interpretation:

- Packet-D improves materially versus baseline and packet-C, but does not beat packet-B aggregate.
- Under authoritative closure policy, packet-D still does **not** satisfy PERF-03 (`qjs-rs <= boa-engine`).

## 4) Command transcript (machine-checkable run bundle)

```bash
cargo test -p vm packet_d_identifier_slot_fast_path_guarding -- --exact
cargo test -p vm perf_packet_d -- --nocapture
cargo test -p benchmarks packet_d_output_path_enables_packet_d_runtime_toggle -- --exact
cargo run -p benchmarks --release -- --profile local-dev --output target/benchmarks/engine-comparison.local-dev.packet-d.json --allow-missing-comparators
cargo run -p benchmarks --release -- --profile ci-linux --output target/benchmarks/engine-comparison.ci-linux.packet-d.json --allow-missing-comparators
python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.local-dev.packet-d.json
python .github/scripts/check_engine_benchmark_contract.py --input target/benchmarks/engine-comparison.ci-linux.packet-d.json
python .github/scripts/check_perf_target.py --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json --candidate target/benchmarks/engine-comparison.local-dev.packet-d.json --require-qjs-lte-boa
```

Command outcomes:

- VM packet-D guard test: ✅
- VM packet-D parity suite: ✅
- Bench packet-D toggle unit test: ✅
- Packet-D local-dev artifact generation: ✅
- Packet-D ci-linux artifact generation: ✅
- Benchmark contract checker (local-dev): ✅
- Benchmark contract checker (ci-linux): ✅
- PERF-03 authoritative checker (`--require-qjs-lte-boa`): ❌

## 5) Closure verdict

- Packet-D identifier-slot implementation + parity/guard telemetry: ✅ complete
- Packet-D contract-valid artifacts + checker transcript: ✅ complete
- PERF-03 closure (`qjs-rs <= boa-engine` on authoritative profile): ❌ still open

Packet-D is a valid evidence candidate but **not** a closure candidate under current policy.
