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

For `qjs-rs` runs, local-dev now enables packet-D fast-path composition by default so baseline iteration loops track the latest stable optimization stack.

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
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.phase11-baseline.json
```

Phase 11 artifacts now embed:

- `perf_target` metadata (closure policy, optimization tag/packet id, host fingerprint, comparator policy)
- optional `qjs_rs_hotspot_attribution` counters for packet-level hotspot auditing

### Phase 11 canonical closure candidate workflow (packet-D example)

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.packet-d.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

cargo run -p benchmarks --release -- \
  --profile ci-linux \
  --output target/benchmarks/engine-comparison.ci-linux.packet-d.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.packet-d.json

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.ci-linux.packet-d.json

python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-d.json \
  --require-qjs-lte-quickjs-ratio 1.25
```

If the final ratio check fails (`qjs-rs/quickjs-c > 1.25`), Phase 11 PERF-03 closure is
**not** satisfied and the run must be recorded as a non-closure candidate.

PERF-04 still requires packet/hotspot evidence publication even when PERF-03 remains open.

Legacy `--require-qjs-lte-boa` checker runs are audit-only and are not active closure criteria.

### Phase 11 packet-C closure candidate workflow

`packet-c` artifacts automatically enable the packet-C runtime toggle for `qjs-rs` runs
(`packet_c_enabled=true` in harness logs) while preserving the same contract/profile policy.

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.packet-c.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

cargo run -p benchmarks --release -- \
  --profile ci-linux \
  --output target/benchmarks/engine-comparison.ci-linux.packet-c.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.packet-c.json

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.ci-linux.packet-c.json

python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-c.json \
  --require-qjs-lte-quickjs-ratio 1.25
```

If the final ratio check fails (`qjs-rs/quickjs-c > 1.25`), PERF-03 remains open and packet-C
must be documented as evidence-only (no closure claim).

### Phase 11 packet-D closure candidate workflow

`packet-d` artifacts automatically enable the packet-D identifier-slot runtime toggle for
`qjs-rs` runs (`packet_d_enabled=true` in harness logs) while preserving the same
contract/profile policy.

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.packet-d.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

cargo run -p benchmarks --release -- \
  --profile ci-linux \
  --output target/benchmarks/engine-comparison.ci-linux.packet-d.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.packet-d.json

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.ci-linux.packet-d.json

python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-d.json \
  --require-qjs-lte-quickjs-ratio 1.25
```

If the final ratio check fails (`qjs-rs/quickjs-c > 1.25`), PERF-03 remains open and packet-D
must be documented as evidence-only (no closure claim).

### Phase 11 packet-E closure candidate workflow

`packet-e` artifacts keep packet-C and packet-D runtime toggles enabled so identifier packet
experiments compose with existing guarded fast paths.

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.packet-e.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --strict-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.packet-e.json

python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-e.json \
  --require-qjs-lte-quickjs-ratio 1.25
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
python .github/scripts/check_perf_target.py --self-test
```

## Notes

- The benchmark suite covers `arith-loop`, `fib-iterative`, `array-sum`, and `json-roundtrip`.
- Contract checker failures are blocking and must be fixed before report generation is accepted.
- Phase 11 perf-target claims are only valid when policy checks pass under the authoritative `local-dev` + `eval-per-iteration` + same-host policy and the aggregate ratio `qjs-rs/quickjs-c <= 1.25`.
- Rendered reports include schema/profile/timing/comparator metadata so audit reviewers can confirm reproducibility context directly from markdown.
