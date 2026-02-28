# Engine Benchmarks (qjs-rs vs boa-engine vs quickjs-c vs nodejs)

Phase 10 contract (`bench.v1`) requires this deterministic execution order for every evidence run:

1. **Run benchmark harness**
2. **Validate JSON contract** with `.github/scripts/check_engine_benchmark_contract.py`
3. **Render markdown + SVG report** from the validated JSON

Skipping step 2 is not allowed for PERF-01/PERF-02 evidence publication.

For Phase 11 target closure rules (PERF-03/PERF-04/PERF-05), see
[`docs/performance-closure-policy.md`](./performance-closure-policy.md).

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

### Phase 11 authoritative baseline (`local-dev`, same-host rerun policy)

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.phase11-baseline.json
```

Phase 11 artifacts now embed:

- `perf_target` metadata (closure policy, optimization tag/packet id, host fingerprint, comparator policy)
- optional `qjs_rs_hotspot_attribution` counters for packet-level hotspot auditing

### Phase 11 packet-B closure candidate workflow

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.packet-b.json \
  --allow-missing-comparators

cargo run -p benchmarks --release -- \
  --profile ci-linux \
  --output target/benchmarks/engine-comparison.ci-linux.packet-b.json \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.packet-b.json

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.ci-linux.packet-b.json

python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-b.json \
  --require-qjs-lte-boa
```

If the final `check_perf_target.py --require-qjs-lte-boa` command fails, Phase 11 PERF-03
closure is **not** satisfied and the run must be recorded as a non-closure candidate.

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
python .github/scripts/check_perf_target.py --self-test
```

## Notes

- The benchmark suite covers `arith-loop`, `fib-iterative`, `array-sum`, and `json-roundtrip`.
- Contract checker failures are blocking and must be fixed before report generation is accepted.
- Phase 11 perf-target claims are only valid when `.github/scripts/check_perf_target.py` passes under the authoritative `local-dev` + `eval-per-iteration` + same-host policy.
- Rendered reports include schema/profile/timing/comparator metadata so audit reviewers can confirm reproducibility context directly from markdown.
