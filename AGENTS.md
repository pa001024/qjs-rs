# PROJECT KNOWLEDGE BASE

**Generated:** 2026-03-03T08:10:46Z
**Commit:** 1237c1a
**Branch:** main

## OVERVIEW
Pure Rust JavaScript runtime workspace, semantics-first and QuickJS-aligned.
Core execution path is `parser -> bytecode -> vm -> runtime -> builtins -> test-harness` with governance + benchmark gates in CI.

## STRUCTURE
```text
qjs-rs/
├── crates/               # first-party engine crates (main implementation surface)
├── docs/                 # policy, compatibility, benchmark contracts
├── .github/              # CI + governance/traceability/benchmark checks
├── scripts/              # local benchmark/render helpers
├── .planning/            # milestone/phase planning artifacts
└── boa/                  # embedded upstream reference tree (not primary implementation)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Workspace graph and toolchain baseline | `Cargo.toml` | `resolver = "2"`, edition `2024`, rust-version `1.85` |
| VM behavior, object model, GC, module lifecycle | `crates/vm/src/lib.rs` | largest hotspot, many invariants + fast-path toggles |
| test262-lite harness, suite execution, GC drift | `crates/test-harness/src/test262.rs` | shared by harness tests and CLI |
| Harness CLI (`test262-run`) | `crates/test-harness/src/bin/test262-run.rs` | baseline/stress profiles + gc expectation checks |
| CI execution order and mandatory gates | `.github/workflows/ci.yml` | single `rust` job with governance + phase gates |
| Governance schema and PR checklist | `.github/governance/README.md`, `.github/PULL_REQUEST_TEMPLATE.md` | test references + exception contract enforced |
| Benchmark artifact contract | `docs/benchmark-contract.md` | `bench.v1` envelope + required engines/cases |
| Benchmark runbook + closure policy | `docs/engine-benchmarks.md`, `docs/performance-closure-policy.md` | run->validate->render + ratio gate |

## CODE MAP
| Symbol | Type | Location | Refs | Role |
|--------|------|----------|------|------|
| `Vm` | struct | `crates/vm/src/lib.rs` | high | execution core and runtime state owner |
| `Vm::execute_in_realm` | method | `crates/vm/src/lib.rs` | high | full state reset + chunk execution entry |
| `ScriptRuntime` | struct | `crates/vm/src/script_runtime.rs` | medium | host-facing script execution wrapper |
| `parse_script` | function | `crates/parser/src/lib.rs` | high | parser entry for scripts |
| `run_suite` | function | `crates/test-harness/src/test262.rs` | high | test262-lite suite driver |

## CONVENTIONS
- Runtime core stays pure Rust; no C FFI in core runtime path.
- CI is policy-first: governance + verification traceability + benchmark contract run before fmt/clippy/test.
- PRs must include positive and boundary/error test references (or explicit refactor-only evidence).
- Benchmark evidence is contract-bound (`bench.v1`), reproducibility metadata is mandatory, silent comparator omission is disallowed.
- Phase closure claims require policy checks (`check_perf_target.py`) and ratio threshold satisfaction.

## ANTI-PATTERNS (THIS PROJECT)
- Introducing runtime-core C FFI dependencies.
- Renaming contract case IDs or adding ad-hoc benchmark cases inside `bench.v1` artifacts.
- Publishing benchmark reports without contract validation step.
- Using implicit long-lived host references instead of explicit register/unregister lifecycle.
- Adding strict-mode parser behavior that permits forbidden binding forms (`eval/arguments` misuse, strict reserved binding names, duplicate strict params).
- Bypassing governance checklist/test reference requirements in PR flows.

## COMMANDS
```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite

python .github/scripts/validate_governance.py --exceptions .github/governance/exceptions.json --check-template .github/PULL_REQUEST_TEMPLATE.md --repo-root . --self-test
python .github/scripts/check_verification_traceability.py --requirements .planning/REQUIREMENTS.md --phases-dir .planning/phases --out-json target/verification-traceability.json --out-md target/verification-traceability.md
python .github/scripts/check_engine_benchmark_contract.py --self-test
```

## NOTES
- `boa/` is an embedded upstream reference corpus; avoid mixing its internal conventions into first-party crate decisions.
- `docs/current-status.md` is generated from snapshot manifest; keep drift checks green in CI.
- `crates/test-harness/fixtures/test262-lite` is a major compatibility asset tree; treat fixture updates as behavior changes.
