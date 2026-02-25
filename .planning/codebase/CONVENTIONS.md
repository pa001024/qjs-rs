# Coding Conventions

**Analysis Date:** 2026-02-25

## Naming Patterns

**Files:**
- Workspace crates use lowercase package names and kebab-style directories such as `crates/test-harness`.
- Most crate entrypoints are `src/lib.rs`, e.g. `crates/parser/src/lib.rs`, `crates/bytecode/src/lib.rs`, `crates/vm/src/lib.rs`.
- Binary entrypoints live in `src/bin`, currently `crates/test-harness/src/bin/test262-run.rs`.
- Integration tests live in `tests/`, currently `crates/test-harness/tests/test262_lite.rs`.

**Functions:**
- Rust function names are `snake_case` (`parse_script`, `compile_script`, `run_suite`, `execute_in_realm`) in `crates/parser/src/lib.rs`, `crates/bytecode/src/lib.rs`, `crates/test-harness/src/test262.rs`, and `crates/vm/src/lib.rs`.
- Test names follow behavior-oriented `snake_case` (for example `lexes_add_expression` in `crates/lexer/src/lib.rs`).

**Variables and Constants:**
- Locals/fields are `snake_case` across all crates.
- Constants are `UPPER_SNAKE_CASE` (for example marker constants in `crates/parser/src/lib.rs` and `crates/vm/src/lib.rs`).
- Internal semantic marker strings use `$__qjs_*` naming in `crates/parser/src/lib.rs` and `crates/vm/src/lib.rs`.

**Types:**
- Structs/enums use PascalCase (`ParseError`, `Opcode`, `VmError`, `SuiteOptions`) in `crates/parser/src/lib.rs`, `crates/bytecode/src/lib.rs`, `crates/vm/src/lib.rs`, and `crates/test-harness/src/test262.rs`.

## Code Style

**Formatting:**
- CI enforces rustfmt with `cargo fmt --check` in `.github/workflows/ci.yml`.
- Current code follows standard rustfmt style (4-space indentation, trailing commas where appropriate) in `crates/*/src/lib.rs`.
- Unsafe code is forbidden by crate-level `#![forbid(unsafe_code)]` in files such as `crates/ast/src/lib.rs`, `crates/lexer/src/lib.rs`, `crates/parser/src/lib.rs`, and `crates/vm/src/lib.rs`.

**Linting:**
- CI enforces `cargo clippy --workspace --all-targets -- -D warnings` in `.github/workflows/ci.yml`.
- New code should be warning-free for lib, bin, and test targets.

## Import Organization

- Imports are explicit and near the top of each file, with grouped `std` imports when needed (example in `crates/parser/src/lib.rs`).
- Workspace crates are imported by crate name (`use parser::...`, `use runtime::...`) in `crates/test-harness/src/lib.rs` and `crates/vm/src/lib.rs`.
- There is no custom import-order linter; keep grouping readable and consistent with the file being edited.

## Error Handling

- Frontend layers use typed errors with message/position fields (`LexError` in `crates/lexer/src/lib.rs`, `ParseError` in `crates/parser/src/lib.rs`).
- Runtime execution uses `Result<_, VmError>` in `crates/vm/src/lib.rs`.
- Harness APIs convert internal errors to strings at boundaries (see `run_expression`/`run_script` in `crates/test-harness/src/lib.rs`).
- `expect(...)` and targeted `panic!(...)` are used for invariants and CLI argument validation (notably in `crates/test-harness/src/bin/test262-run.rs`).

## Logging

- No dedicated logging crate is wired into the runtime path.
- Optional stage traces use `println!` gated by `QJS_TRACE_STAGES` in `crates/test-harness/src/test262.rs`.
- Human-readable run summaries are emitted by the CLI in `crates/test-harness/src/bin/test262-run.rs`.

## Comments

- Comments are sparse and explain intent or compatibility notes (for example in `crates/builtins/src/lib.rs` and `crates/bytecode/src/lib.rs`).
- No active `TODO`/`FIXME` markers were found under `crates/` during this scan.
- Prefer comments for semantic reasoning over line-by-line narration.

## Function and Module Design

- Public APIs are thin orchestration points (`parse_*`, `compile_*`, `run_*`) in `crates/parser/src/lib.rs`, `crates/bytecode/src/lib.rs`, and `crates/test-harness/src/lib.rs`.
- Large components currently remain single-file implementations (`crates/parser/src/lib.rs`, `crates/vm/src/lib.rs`), so local helper extraction is preferred before cross-file splitting.
- Testing stays close to implementation via `#[cfg(test)] mod tests` plus integration-level checks in `crates/test-harness/tests/test262_lite.rs`.

---

*Convention analysis: 2026-02-25*
*Update when repository patterns change*
