# Testing Patterns

**Analysis Date:** 2026-02-25

## Test Framework

**Runner:**
- Rust built-in test harness via `cargo test` for the workspace in `Cargo.toml`.
- CI executes tests with `cargo test --workspace` in `.github/workflows/ci.yml`.

**Assertion Library:**
- Standard Rust assertion macros (`assert_eq!`, `assert!`, `matches!`, `expect_err`) are used in `crates/*/src/lib.rs` tests.
- No external assertion crate is configured in `Cargo.toml` files.

**Run Commands:**
```bash
cargo test --workspace
cargo test -p parser
cargo test -p vm --lib
cargo test -p test-harness test262_lite
cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite
cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline
```

## Test File Organization

**Location:**
- Most tests are colocated in `#[cfg(test)] mod tests` blocks inside source files such as `crates/lexer/src/lib.rs`, `crates/parser/src/lib.rs`, `crates/bytecode/src/lib.rs`, and `crates/vm/src/lib.rs`.
- Integration tests currently exist in `crates/test-harness/tests/test262_lite.rs`.
- Compatibility fixtures are JS files under `crates/test-harness/fixtures/test262-lite/pass` and `crates/test-harness/fixtures/test262-lite/fail`.

**Current footprint (scan result):**
- `crates/vm/src/lib.rs` has a large unit/regression suite.
- `crates/parser/src/lib.rs`, `crates/lexer/src/lib.rs`, and `crates/bytecode/src/lib.rs` also contain extensive inline tests.
- Workspace currently contains 600+ `#[test]` functions across `crates/`.

## Test Structure

**Suite Organization:**
- Typical layout: `#[cfg(test)] mod tests { use super::...; #[test] fn ... { ... } }` in files like `crates/parser/src/lib.rs` and `crates/bytecode/src/lib.rs`.
- Helper functions are local to test modules when needed (example: `empty_chunk` in `crates/vm/src/lib.rs`).
- Test names describe observable behavior (`parses_*`, `compiles_*`, `evaluates_*`, `supports_*`).

**Patterns:**
- Deterministic assertions are preferred over snapshots.
- Parser/lexer tests compare full AST/token structures directly (`crates/parser/src/lib.rs`, `crates/lexer/src/lib.rs`).
- Runtime tests assert concrete `JsValue` results via `run_expression`/`run_script` helpers in `crates/test-harness/src/lib.rs`.

## Mocking

- No mocking framework is used.
- Tests mostly exercise real parser -> bytecode -> vm paths through helpers in `crates/test-harness/src/lib.rs`.
- External I/O boundaries are primarily in harness/CLI code (`crates/test-harness/src/test262.rs`, `crates/test-harness/src/bin/test262-run.rs`), with direct behavior assertions instead of mocks.

## Fixtures and Factories

- Test262-like fixtures are file-based and frontmatter-driven in `crates/test-harness/fixtures/test262-lite`.
- Frontmatter parsing and suite execution logic are tested in `crates/test-harness/src/test262.rs`.
- Reusable Rust-side setup helpers are embedded in test modules (for example `empty_chunk` in `crates/vm/src/lib.rs`).

## Coverage

- No explicit line/branch coverage threshold is configured in CI.
- Quality gates rely on formatting/lint/test pass criteria in `.github/workflows/ci.yml`.
- Compatibility breadth is tracked via test262-lite and targeted test262 runs documented in `docs/current-status.md` and `docs/test262-baseline.md`.

## Test Types

**Unit Tests:**
- Core semantic tests are colocated with implementation in each crate’s `src/lib.rs`.

**Integration Tests:**
- Cross-layer integration is exercised through `crates/test-harness/src/lib.rs` and `crates/test-harness/tests/test262_lite.rs`.

**Compatibility/Stress Tests:**
- `test262-run` CLI validates fixture suites and GC expectations in `crates/test-harness/src/bin/test262-run.rs`.
- CI includes a GC guard stress gate command in `.github/workflows/ci.yml`.

## Common Patterns

- Happy-path assertions: `assert_eq!(..., Ok(JsValue::...))` in `crates/test-harness/src/lib.rs`.
- Error-path assertions: `expect_err(...)` + `assert!(err.contains(...))` in `crates/test-harness/src/lib.rs` and `crates/test-harness/src/bin/test262-run.rs`.
- Compatibility execution path: parse/compile/execute with per-case expected outcome handling in `crates/test-harness/src/test262.rs`.

---

*Testing analysis: 2026-02-25*
*Update when test layout or gates change*
