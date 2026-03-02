# Phase 11 Performance Closure Policy (PERF-03)

This document is the authoritative closure gate for **Phase 11** target claims.

If a benchmark claim does not satisfy this policy, it **cannot** be used as PERF-03 evidence.

## Authoritative Closure Profile

Phase 11 closure uses exactly one profile/mode pair:

- **Run profile:** `local-dev`
- **Timing mode:** `eval-per-iteration`
- **Host policy:** baseline and candidate must be produced on the **same host rerun**

`check_perf_target.py` enforces these requirements using artifact metadata:

- `run_profile == local-dev`
- `timing_mode == eval-per-iteration`
- `perf_target.same_host_required == true`
- `perf_target.host_fingerprint` matches between baseline and candidate

## Comparator Availability Policy

Baseline/candidate artifacts must preserve metadata comparators (`qjs-rs`, `boa-engine` required; `quickjs-c`, `nodejs` optional) and explicit unavailable reasons.

For the active PERF-03 closure command (`--require-qjs-lte-quickjs-ratio 1.25`), `quickjs-c` becomes mandatory: both artifacts must report `quickjs-c` comparator status as `available`, and both artifacts must include aggregate `quickjs-c` means.

Legacy `--require-qjs-lte-boa` checks remain compatibility-only and are not an active closure criterion.

## Required Artifact Metadata

Each Phase 11 artifact must include a `perf_target` block containing:

- policy identifier
- authoritative profile + timing mode
- optimization mode/tag (+ optional packet id)
- comparator policy (required/optional engines)
- same-host fingerprint metadata

Artifacts may also include `qjs_rs_hotspot_attribution` snapshots for packet-level hotspot auditing.

## Gate Commands

### 1) Baseline creation + contract validation

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.phase11-baseline.json
```

### 2) Candidate creation + contract validation

```bash
cargo run -p benchmarks --release -- \
  --profile local-dev \
  --output target/benchmarks/engine-comparison.local-dev.packet-d.json \
  --quickjs-path scripts/quickjs-wsl.cmd \
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.packet-d.json
```

### 3) PERF-03 ratio gate (authoritative closure check)

```bash
python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-d.json \
  --require-qjs-lte-quickjs-ratio 1.25
```

### 4) Checker self-test (required before policy changes)

```bash
python .github/scripts/check_perf_target.py --self-test
```

## Acceptance Threshold

PERF-03 closure is satisfied only when:

1. baseline/candidate satisfy the policy metadata checks above, and
2. Candidate aggregate latency satisfies `qjs-rs <= 1.25x quickjs-c` (equivalent to >=80% of quickjs-c performance) under the authoritative profile/mode.

No alternate profile, timing mode, or cross-host run is accepted for Phase 11 closure claims.
