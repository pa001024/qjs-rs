# Phase 11 Packet-A Evidence (11-02)

**Date:** 2026-02-28  
**Baseline artifact:** `target/benchmarks/engine-comparison.local-dev.phase11-baseline.json`  
**Candidate artifact:** `target/benchmarks/engine-comparison.local-dev.packet-a.json`

## Command Log

```bash
cargo test -p benchmarks perf_packet_a_report -- --nocapture

cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.packet-a.json \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.packet-a.json

python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-a.json \
  --expect-case-improvement arith-loop \
  --expect-case-improvement fib-iterative \
  --max-case-regression json-roundtrip=1.10
```

## Contract / Policy Gate Results

- `perf_packet_a_report` test: ✅ passed
- benchmark contract checker (`check_engine_benchmark_contract.py`): ✅ passed
- perf-target delta checker (`check_perf_target.py` with packet-A expectations): ✅ passed

## qjs-rs Baseline vs Packet-A Delta

| Case | Baseline mean (ms) | Packet-A mean (ms) | Candidate / Baseline | Delta |
|---|---:|---:|---:|---:|
| `arith-loop` | 2153.732 | 1940.619 | 0.901x | **-9.90%** |
| `fib-iterative` | 162.745 | 145.908 | 0.897x | **-10.35%** |
| `json-roundtrip` | 22.655 | 21.016 | 0.928x | **-7.23%** |
| `array-sum` | 4798.761 | 3071.256 | 0.640x | -35.99% |

Aggregate qjs-rs mean:

- baseline: **1784.473 ms**
- packet-a: **1294.700 ms**
- delta: **-27.45%**

## Packet-A Metadata Confirmation

The packet artifact carries packet labeling in perf-target metadata:

- `perf_target.optimization_mode = "packet"`
- `perf_target.optimization_tag = "packet-a"`
- `perf_target.packet_id = "packet-a"`

This was verified via the dedicated `perf_packet_a_report` test and contract validation.
