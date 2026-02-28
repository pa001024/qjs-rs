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

For closure checks:

- **Required comparators:** `qjs-rs`, `boa-engine` (must be `available`)
- **Optional comparators:** `quickjs-c`, `nodejs` (may be `missing`/`unsupported`)

Optional comparators may be unavailable only if benchmark metadata includes explicit status + reason.
Claims with silent comparator absence are rejected.

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
  --allow-missing-comparators

python .github/scripts/check_engine_benchmark_contract.py \
  --input target/benchmarks/engine-comparison.local-dev.phase11-baseline.json
```

### 2) Candidate comparison gate

```bash
python .github/scripts/check_perf_target.py \
  --baseline target/benchmarks/engine-comparison.local-dev.phase11-baseline.json \
  --candidate target/benchmarks/engine-comparison.local-dev.packet-b.json \
  --require-qjs-lte-boa
```

### 3) Checker self-test (required before policy changes)

```bash
python .github/scripts/check_perf_target.py --self-test
```

## Acceptance Threshold

PERF-03 closure is satisfied only when:

1. baseline/candidate satisfy the policy metadata checks above, and
2. `--require-qjs-lte-boa` passes (candidate aggregate `qjs-rs <= boa-engine`) under the authoritative profile/mode.

No alternate profile, timing mode, or cross-host run is accepted for Phase 11 closure claims.
