# Benchmark Contract (`bench.v1`)

This document defines the canonical benchmark artifact contract for Phase 10.
All benchmark producers and consumers MUST follow this contract.

## 1. Contract Envelope

Every benchmark JSON artifact MUST include:

- `schema_version`: fixed string `"bench.v1"`
- `generated_at_utc`: ISO-8601 UTC timestamp
- `run_profile`: one of `local-dev` or `ci-linux`
- `timing_mode`: enum defined in this document
- `config`: run controls (`iterations`, `samples`, `warmup_iterations`)
- `reproducibility`: reproducibility metadata block
- `environment`: host/runtime metadata block
- `cases`: benchmark case reports (contract-locked required IDs)
- `aggregate`: aggregate engine metrics

## 2. Required Engines

Artifacts MUST track this required engine set (stable IDs):

1. `qjs-rs`
2. `boa-engine`
3. `quickjs-c`
4. `nodejs`

Rules:

- The `reproducibility.required_engines` list MUST include all four IDs.
- Engine metrics in `cases[*].engines` MAY be absent only when engine execution is unsupported in the current environment.
- Missing/unsupported engines MUST be reported explicitly in `reproducibility.engine_status` with:
  - `engine`
  - `status` (`available` | `missing` | `unsupported`)
  - `reason` (mandatory for non-`available`)
- Silent omission is forbidden.

## 3. Required Benchmark Case Catalog

The required hot-path case set is contract-owned and immutable by default:

1. `arith-loop`
2. `fib-iterative`
3. `array-sum`
4. `json-roundtrip`

Rules:

- Case IDs are stable identifiers and MUST NOT be renamed without a schema version bump.
- All required IDs MUST appear exactly once in `cases`.
- Additional experimental cases are disallowed in `bench.v1` artifacts used for baseline comparison.

## 4. Run Profiles

Supported run profiles:

- `local-dev`
  - Defaults: `iterations=200`, `samples=7`, `warmup_iterations=3`
  - Intended for local tuning and quick comparability checks
- `ci-linux`
  - Defaults: `iterations=400`, `samples=9`, `warmup_iterations=5`
  - Intended for reproducible CI/non-regression baselines

Rules:

- CLI overrides (`--iterations`, `--samples`, `--warmup-iterations`) are allowed.
- Effective values MUST be serialized in `config` for reproducibility.

## 5. Timing Mode Enum

`timing_mode` MUST be one of:

- `eval-per-iteration` — evaluate benchmark script for each inner iteration

`bench.v1` requires `eval-per-iteration` for all engines in a run.

## 6. Deterministic Artifact Naming

Default output path MUST be profile-derived:

- `target/benchmarks/engine-comparison.<profile>.json`

Examples:

- `target/benchmarks/engine-comparison.local-dev.json`
- `target/benchmarks/engine-comparison.ci-linux.json`

Rules:

- Producers MAY accept `--output` for ad-hoc runs.
- If overridden, `reproducibility.output_policy.default_path` MUST still record canonical default.

## 7. Mandatory Reproducibility Metadata

`reproducibility` MUST include at least:

- `required_engines`: canonical engine list
- `required_case_ids`: canonical case ID list
- `output_policy.default_path`: contract default artifact path for the selected profile
- `engine_status`: per-engine availability/unsupported diagnostics

`environment` MUST include at least:

- `os`
- `arch`
- `cpu_parallelism`
- `rustc`
- `node`
- `quickjs_c`

## 8. Compatibility Rules

- Contract drift in envelope fields, required engine IDs, required case IDs, or run-profile enum requires explicit code review and test updates.
- Any breaking envelope change requires a new schema version (for example `bench.v2`).