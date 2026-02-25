# Technology Stack

**Analysis Date:** 2026-02-25

## Languages

**Primary:**
- Rust 2024 edition - Engine/runtime implementation across workspace crates (`Cargo.toml`, `crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`, `crates/parser/src/lib.rs`).

**Secondary:**
- JavaScript (test corpus and fixtures) - Compatibility suite inputs (`crates/test-harness/fixtures/test262-lite/pass/arithmetic.js`, `crates/test-harness/fixtures/test262-lite/fail/parse/throw-no-expression.js`).
- YAML - CI pipeline configuration (`.github/workflows/ci.yml`).
- Markdown - Project status/planning docs (`docs/current-status.md`, `docs/test262-lite.md`).

## Runtime

**Environment:**
- Rust toolchain (stable in CI) (`.github/workflows/ci.yml` uses `dtolnay/rust-toolchain@stable`).
- Minimum Rust version: 1.85 (`Cargo.toml` under `[workspace.package].rust-version`).
- Unsafe code is disallowed in core crates (`crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`, `crates/test-harness/src/test262.rs` with `#![forbid(unsafe_code)]`).

**Package Manager:**
- Cargo workspace builds all crates (`Cargo.toml` with `[workspace]` members).
- Lockfile present and committed: `Cargo.lock`.

## Frameworks

**Core:**
- No external application framework; this is a pure Rust engine library workspace (`Cargo.toml`, `crates/*/Cargo.toml`).
- Layered internal crates for pipeline execution (`crates/lexer/src/lib.rs` -> `crates/parser/src/lib.rs` -> `crates/bytecode/src/lib.rs` -> `crates/vm/src/lib.rs` -> `crates/runtime/src/lib.rs` -> `crates/builtins/src/lib.rs`).

**Testing:**
- Rust built-in test framework via `cargo test` (tests colocated in `crates/test-harness/src/lib.rs`, `crates/test-harness/src/test262.rs`, `crates/test-harness/tests/test262_lite.rs`).
- Custom test262 runner binary for compatibility sweeps (`crates/test-harness/src/bin/test262-run.rs`).

**Build/Dev:**
- Formatting and linting through rustfmt and clippy (`.github/workflows/ci.yml` runs `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`).
- Workspace-wide test execution in CI (`.github/workflows/ci.yml` runs `cargo test --workspace`).

## Key Dependencies

**Critical:**
- `fancy-regex` 0.14.0 - RegExp execution backend in VM (`crates/vm/Cargo.toml`, `crates/vm/src/lib.rs`, `Cargo.lock`).
- `regex` 1.12.3 - Regex utilities and validation paths in VM/tests (`crates/vm/Cargo.toml`, `crates/vm/src/lib.rs`, `Cargo.lock`).
- Internal crate `ast` - Shared syntax model (`crates/ast/src/lib.rs`, `crates/parser/Cargo.toml`, `crates/bytecode/Cargo.toml`).
- Internal crate `bytecode` - Opcode/chunk layer (`crates/bytecode/src/lib.rs`, `crates/vm/Cargo.toml`).
- Internal crate `runtime` - Core value/realm types (`crates/runtime/src/lib.rs`, `crates/vm/Cargo.toml`, `crates/builtins/Cargo.toml`).

**Infrastructure:**
- `regex-automata`, `regex-syntax`, `aho-corasick`, `memchr`, `bit-set`, `bit-vec` transitively required by regex stack (`Cargo.lock`).

## Configuration

**Environment:**
- Optional tracing env vars for harness execution: `QJS_TRACE_STAGES`, `QJS_TRACE_CASES` (`crates/test-harness/src/test262.rs`).
- No `.env` template or runtime secret configuration files in repo root (`Cargo.toml`, `.github/workflows/ci.yml`, `crates/*/Cargo.toml`).

**Build:**
- Workspace/package metadata and crate graph in `Cargo.toml`.
- Dependency pinning in `Cargo.lock`.
- CI policy and quality gates in `.github/workflows/ci.yml`.

## Platform Requirements

**Development:**
- Any OS with Rust >= 1.85 and Cargo (validated in CI on `ubuntu-latest` via `.github/workflows/ci.yml`).
- Local filesystem access needed for fixture/test262 roots (`crates/test-harness/src/bin/test262-run.rs`, `crates/test-harness/src/test262.rs`).

**Production:**
- Delivered as Rust library crates plus optional CLI tooling (`crates/test-harness/src/bin/test262-run.rs`).
- No container/orchestrator or cloud deployment configuration present (`.github/workflows/ci.yml`, `Cargo.toml`).

---

*Stack analysis: 2026-02-25*
*Update after major dependency changes*
