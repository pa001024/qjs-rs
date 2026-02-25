# Architecture

**Analysis Date:** 2026-02-25

## Pattern Overview

**Overall:** Layered Rust workspace engine with a script execution pipeline (`lexer/parser -> ast -> bytecode -> vm/runtime`) plus a dedicated compatibility harness.

**Key Characteristics:**
- Single Cargo workspace rooted at `Cargo.toml` with focused engine crates under `crates/`.
- Parser/compiler/runtime boundaries are explicit through crate dependencies in `crates/*/Cargo.toml`.
- Runtime semantics are centralized in `crates/vm/src/lib.rs` with value types in `crates/runtime/src/lib.rs`.
- Built-in globals are installed via `crates/builtins/src/lib.rs` before execution.
- Validation and compatibility are driven through `crates/test-harness/src/lib.rs` and `crates/test-harness/src/test262.rs`.

## Layers

**Lexing and Parsing Layer:**
- Purpose: Convert JavaScript source to typed AST with early-error and strict-mode checks.
- Contains: `lex()` in `crates/lexer/src/lib.rs`, `parse_expression()` and `parse_script()` in `crates/parser/src/lib.rs`, AST types in `crates/ast/src/lib.rs`.
- Depends on: `crates/lexer` depends only on std; `crates/parser` depends on `crates/lexer` and `crates/ast`.
- Used by: `crates/bytecode/src/lib.rs` and `crates/test-harness/src/lib.rs`.

**Compilation Layer:**
- Purpose: Lower AST to bytecode chunk and function table.
- Contains: `Opcode`, `Chunk`, `CompiledFunction`, `compile_script()`, `compile_expression()` in `crates/bytecode/src/lib.rs`.
- Depends on: `crates/ast`.
- Used by: `crates/vm/src/lib.rs` and harness execution paths in `crates/test-harness/src/lib.rs`.

**Value Model Layer:**
- Purpose: Define engine-level value representations and global namespace container.
- Contains: `JsValue`, `NativeFunction`, `Realm` in `crates/runtime/src/lib.rs`.
- Depends on: std only.
- Used by: `crates/vm/src/lib.rs`, `crates/builtins/src/lib.rs`, and `crates/test-harness/src/lib.rs`.

**Execution and Semantics Layer:**
- Purpose: Execute bytecode, maintain scopes/objects/functions, implement built-in behavior, and run GC.
- Contains: `Vm`, `VmError`, `GcStats`, `execute_in_realm()`, native/host dispatch in `crates/vm/src/lib.rs`.
- Depends on: `crates/runtime`, `crates/bytecode`, `crates/parser`, plus `fancy-regex`/`regex` from `crates/vm/Cargo.toml`.
- Used by: `crates/test-harness/src/lib.rs` and test suite execution in `crates/test-harness/src/test262.rs`.

**Builtins Installation Layer:**
- Purpose: Seed realm globals (`Object`, `Array`, `Date`, `Map`, `Set`, `Promise`, `RegExp`, etc.) for runtime execution.
- Contains: `install_baseline()` in `crates/builtins/src/lib.rs`.
- Depends on: `crates/runtime`.
- Used by: `crates/test-harness/src/lib.rs` and `crates/test-harness/src/test262.rs`.

**Compatibility Harness Layer:**
- Purpose: Provide reusable run helpers and test262-lite suite orchestration with outcome/GC summaries.
- Contains: `run_expression()`, `run_script()` in `crates/test-harness/src/lib.rs`; `run_suite()` and frontmatter parsing in `crates/test-harness/src/test262.rs`; CLI in `crates/test-harness/src/bin/test262-run.rs`.
- Depends on: `crates/parser`, `crates/bytecode`, `crates/vm`, `crates/runtime`, `crates/builtins`.
- Used by: CI at `.github/workflows/ci.yml` and crate tests in `crates/test-harness/tests/test262_lite.rs`.

## Data Flow

**Expression/Script Execution (`run_expression` / `run_script`):**

1. Source text enters `run_expression()` or `run_script()` in `crates/test-harness/src/lib.rs`.
2. Parser stage builds AST via `parse_expression()` or `parse_script()` from `crates/parser/src/lib.rs`.
3. Compiler stage lowers AST to `Chunk` with `compile_expression()` or `compile_script()` in `crates/bytecode/src/lib.rs`.
4. Realm stage creates `Realm` and installs builtins using `install_baseline()` in `crates/builtins/src/lib.rs`.
5. VM stage runs `Vm::execute_in_realm()` in `crates/vm/src/lib.rs` and returns `JsValue` or `VmError`.
6. Harness maps VM errors to string form and returns API-friendly `Result` in `crates/test-harness/src/lib.rs`.

**Suite Execution (`test262-run`):**

1. CLI parses options in `main()` at `crates/test-harness/src/bin/test262-run.rs`.
2. Suite discovery and case parsing occur in `run_suite()` and `parse_test262_case()` in `crates/test-harness/src/test262.rs`.
3. Each case executes parse/compile/vm pipeline with optional GC toggles (`--auto-gc`, `--runtime-gc`) from `crates/test-harness/src/bin/test262-run.rs`.
4. Results aggregate into `SuiteSummary`/`SuiteGcSummary` in `crates/test-harness/src/test262.rs`.
5. CI gate consumes this path in `.github/workflows/ci.yml` (GC baseline expectation check).

**State Management:**
- Global identifiers live in `Realm` maps in `crates/runtime/src/lib.rs`.
- Runtime object/function/scope state and GC bookkeeping live in `Vm` in `crates/vm/src/lib.rs`.
- Harness-level state is per run; `run_suite()` aggregates immutable summary outputs in `crates/test-harness/src/test262.rs`.

## Key Abstractions

**Runtime Value Model (`JsValue`):**
- Purpose: Uniform value carrier for primitives, callable handles, and object handles.
- Examples: `JsValue::Number`, `JsValue::Object`, `JsValue::NativeFunction` in `crates/runtime/src/lib.rs`.
- Pattern: Tagged enum + handle indirection.

**Bytecode Contract (`Opcode`/`Chunk`):**
- Purpose: Stable execution contract between compiler and VM.
- Examples: `Opcode::Call`, `Opcode::DefineVariable`, `Opcode::Throw` in `crates/bytecode/src/lib.rs`.
- Pattern: Stack-machine instruction stream with function table.

**Execution Core (`Vm`):**
- Purpose: Interpret bytecode and enforce JS semantics.
- Examples: `Vm::execute_in_realm()`, `Vm::collect_garbage()`, `Vm::gc_stats()` in `crates/vm/src/lib.rs`.
- Pattern: Stateful interpreter with explicit environment/object stores.

**Conformance Summary (`SuiteSummary`):**
- Purpose: Capture pass/fail and GC telemetry for regression tracking.
- Examples: `SuiteSummary`, `SuiteGcSummary` in `crates/test-harness/src/test262.rs`.
- Pattern: Aggregation DTOs returned by `run_suite()`.

## Entry Points

**Library Execution API:**
- Location: `crates/test-harness/src/lib.rs`.
- Triggers: Internal tests or embedding code calling `run_expression()` / `run_script()`.
- Responsibilities: Parse, compile, initialize realm, execute VM, normalize errors.

**Suite CLI Entry:**
- Location: `crates/test-harness/src/bin/test262-run.rs`.
- Triggers: `cargo run -p test-harness --bin test262-run -- ...`.
- Responsibilities: Parse flags, run suite, print/report failures, enforce GC thresholds.

**Parser API Entry:**
- Location: `crates/parser/src/lib.rs`.
- Triggers: Compiler/harness invocation of `parse_expression()` and `parse_script()`.
- Responsibilities: Tokenize, parse, run strict/early validations, return AST or parse errors.

## Error Handling

**Strategy:** Layer-local typed errors, converted to user-facing strings at harness boundary.

**Patterns:**
- `parse_*` returns `Result<_, ParseError>` in `crates/parser/src/lib.rs`.
- VM execution returns `Result<JsValue, VmError>` in `crates/vm/src/lib.rs`.
- Harness converts VM errors with `format!("{err:?}")` in `crates/test-harness/src/lib.rs`.
- Suite runner maps parse/runtime mismatch into `ExecutionOutcome` and `FailureDetail` in `crates/test-harness/src/test262.rs`.

## Cross-Cutting Concerns

**Conformance and Regression Safety:**
- CI gates `cargo fmt`, `cargo clippy`, `cargo test`, and GC stress baseline at `.github/workflows/ci.yml`.
- test262-lite fixture corpus lives under `crates/test-harness/fixtures/test262-lite`.

**GC Observability:**
- Runtime exposes `GcStats` in `crates/vm/src/lib.rs`.
- Harness surfaces aggregate GC stats via `SuiteGcSummary` in `crates/test-harness/src/test262.rs`.
- CLI supports expectation flags such as `--expect-gc-baseline` in `crates/test-harness/src/bin/test262-run.rs`.

**Strict/Early Validation:**
- Parser enforces strict-mode restrictions and early errors in `crates/parser/src/lib.rs`.
- Validation is upstream of bytecode generation to keep VM focused on execution semantics.

---

*Architecture analysis: 2026-02-25*
*Update when major execution patterns or crate boundaries change*
