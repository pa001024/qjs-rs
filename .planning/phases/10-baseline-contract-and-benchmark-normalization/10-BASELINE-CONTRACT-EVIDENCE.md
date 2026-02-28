# Phase 10 Baseline Contract Evidence Procedure

This note defines the reproducible command sequence required to claim Phase 10 (`PERF-01`, `PERF-02`) closure evidence.

## 1) Generate deterministic benchmark artifact

### local-dev profile

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.json \
  --allow-missing-comparators
```

### ci-linux profile

```bash
cargo run -p benchmarks --release -- \
  --profile ci-linux \
  --output target/benchmarks/engine-comparison.ci-linux.json \
  --strict-comparators
```

## 2) Validate contract before publishing evidence

```bash
python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.<profile>.json
```

`<profile>` must be either `local-dev` or `ci-linux`.

## 3) Render human-readable report only after contract pass

```bash
python scripts/render_engine_benchmark_report.py \
  --input target/benchmarks/engine-comparison.<profile>.json \
  --chart target/benchmarks/engine-comparison.<profile>.svg \
  --report target/benchmarks/engine-comparison.<profile>.md
```

## Fast CI smoke alternative (no live benchmark execution)

```bash
python .github/scripts/check_engine_benchmark_contract.py --self-test
python .github/scripts/check_engine_benchmark_contract.py \
  --input .github/scripts/benchmark_contract/fixtures/benchmark-report-valid.json
```

## Expected Evidence Files

For each profile execution, baseline evidence is considered complete only if all files exist:

- `target/benchmarks/engine-comparison.<profile>.json`
- `target/benchmarks/engine-comparison.<profile>.svg`
- `target/benchmarks/engine-comparison.<profile>.md`

The JSON artifact must pass `check_engine_benchmark_contract.py` with zero errors before the markdown/SVG outputs are accepted for audit.
