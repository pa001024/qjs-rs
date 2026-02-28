# Engine Benchmarks (qjs-rs vs boa-engine vs quickjs-c vs nodejs)

Phase 10 contract (`bench.v1`) requires this deterministic execution order for every evidence run:

1. **Run benchmark harness**
2. **Validate JSON contract** with `.github/scripts/check_engine_benchmark_contract.py`
3. **Render markdown + SVG report** from the validated JSON

Skipping step 2 is not allowed for PERF-01/PERF-02 evidence publication.

## Deterministic Artifact Paths

| Profile | JSON Artifact | Chart Artifact | Markdown Artifact |
| --- | --- | --- | --- |
| `local-dev` | `target/benchmarks/engine-comparison.local-dev.json` | `target/benchmarks/engine-comparison.local-dev.svg` | `target/benchmarks/engine-comparison.local-dev.md` |
| `ci-linux` | `target/benchmarks/engine-comparison.ci-linux.json` | `target/benchmarks/engine-comparison.ci-linux.svg` | `target/benchmarks/engine-comparison.ci-linux.md` |

## Local Reproducibility Workflow (`local-dev`)

`local-dev` defaults: `iterations=200`, `samples=7`, `warmup_iterations=3`, lenient comparators.

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.json \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.json

python scripts/render_engine_benchmark_report.py \
  --input target/benchmarks/engine-comparison.local-dev.json \
  --chart target/benchmarks/engine-comparison.local-dev.svg \
  --report target/benchmarks/engine-comparison.local-dev.md
```

## CI Baseline Workflow (`ci-linux`)

`ci-linux` defaults: `iterations=400`, `samples=9`, `warmup_iterations=5`, strict comparators.

```bash
cargo run -p benchmarks --release -- \
  --profile ci-linux \
  --output target/benchmarks/engine-comparison.ci-linux.json \
  --strict-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.ci-linux.json

python scripts/render_engine_benchmark_report.py \
  --input target/benchmarks/engine-comparison.ci-linux.json \
  --chart target/benchmarks/engine-comparison.ci-linux.svg \
  --report target/benchmarks/engine-comparison.ci-linux.md
```

## Fast CI Contract Smoke Path

For quick CI gates that should avoid full benchmark runtime, run the checker against deterministic fixtures:

```bash
python .github/scripts/check_engine_benchmark_contract.py --self-test
python .github/scripts/check_engine_benchmark_contract.py \
  --input .github/scripts/benchmark_contract/fixtures/benchmark-report-valid.json
```

## Notes

- The benchmark suite covers `arith-loop`, `fib-iterative`, `array-sum`, and `json-roundtrip`.
- Contract checker failures are blocking and must be fixed before report generation is accepted.
- Rendered reports include schema/profile/timing/comparator metadata so audit reviewers can confirm reproducibility context directly from markdown.
