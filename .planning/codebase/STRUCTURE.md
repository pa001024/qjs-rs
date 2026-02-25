# Codebase Structure

**Analysis Date:** 2026-02-25

## Directory Layout

```text
qjs-rs/
├── .github/                         # CI configuration
│   └── workflows/                   # GitHub Actions workflow files
├── .planning/
│   └── codebase/                    # Codebase mapping docs (this folder)
├── agents/                          # Role/instruction docs for collaborators
├── crates/                          # Rust workspace crates (engine + harness)
│   ├── ast/                         # Shared AST node definitions
│   ├── lexer/                       # Tokenizer
│   ├── parser/                      # Parser + early checks
│   ├── bytecode/                    # AST to opcode compiler
│   ├── runtime/                     # Runtime value and realm model
│   ├── vm/                          # VM interpreter + GC + builtins semantics
│   ├── builtins/                    # Baseline global installation
│   └── test-harness/                # Run helpers + test262-lite harness + CLI
├── docs/                            # Roadmap/status/design documents
├── skills/                          # Local skill bundle for this repo
├── target/                          # Build outputs and local snapshots (generated)
├── AGENTS.md                        # Repository-wide agent instructions
├── Cargo.toml                       # Workspace manifest
├── Cargo.lock                       # Dependency lockfile
└── .gitignore                       # Ignore rules (includes `target/`)
```

## Directory Purposes

**`crates/`:**
- Purpose: Houses the executable engine pipeline as separate crates.
- Contains: `Cargo.toml` + `src/lib.rs` in each crate, plus crate-local tests and fixtures.
- Key files: `crates/vm/src/lib.rs`, `crates/parser/src/lib.rs`, `crates/bytecode/src/lib.rs`.
- Subdirectories: `crates/test-harness/src/bin/` for CLI and `crates/test-harness/fixtures/test262-lite/` for conformance fixtures.

**`docs/`:**
- Purpose: Project status, risk, migration, and design records.
- Contains: Markdown docs such as `docs/current-status.md`, `docs/quickjs-mapping.md`, `docs/semantics-checklist.md`.
- Key files: `docs/current-status.md`, `docs/risk-register.md`.
- Subdirectories: None (flat markdown set at present).

**`.github/workflows/`:**
- Purpose: Continuous integration automation.
- Contains: Workflow YAML files.
- Key files: `.github/workflows/ci.yml`.
- Subdirectories: None currently beyond workflow folder.

**`agents/`:**
- Purpose: Collaboration and role reference for specialized workers.
- Contains: Agent instruction markdown.
- Key files: `agents/qjs-rust-porter.md`.
- Subdirectories: None currently.

**`skills/`:**
- Purpose: Repo-local skill definitions and scripts.
- Contains: Skill package under `skills/qjs-rs-runtime-migration/`.
- Key files: `skills/qjs-rs-runtime-migration/SKILL.md`, `skills/qjs-rs-runtime-migration/scripts/inventory_sources.ps1`.
- Subdirectories: `agents/`, `references/`, `scripts/`.

**`target/`:**
- Purpose: Generated build artifacts and local analysis snapshots.
- Contains: Rust build outputs and many temporary test result files.
- Key files: Generated only (example snapshot names appear under `target/`).
- Subdirectories: Build trees such as `target/debug/` and temporary folders.

## Key File Locations

**Entry Points:**
- `crates/test-harness/src/lib.rs`: Public run APIs (`run_expression`, `run_script`) for end-to-end execution.
- `crates/test-harness/src/bin/test262-run.rs`: CLI entry (`main`) for suite execution and GC gates.
- `crates/parser/src/lib.rs`: Parse API entry for expression/script parsing.

**Configuration:**
- `Cargo.toml`: Workspace member list and shared package/dependency configuration.
- `.github/workflows/ci.yml`: Format/lint/test and GC stress gate pipeline.
- `.gitignore`: Excludes `target/` and lockfile policy (`Cargo.lock` is ignored in this repo).

**Core Logic:**
- `crates/ast/src/lib.rs`: AST structures for expressions/statements.
- `crates/lexer/src/lib.rs`: Tokenization with comments/regexp/template handling.
- `crates/bytecode/src/lib.rs`: Opcode definitions and compiler.
- `crates/runtime/src/lib.rs`: `JsValue`, `NativeFunction`, and `Realm`.
- `crates/vm/src/lib.rs`: Interpreter, native behavior, objects/scopes, GC.
- `crates/builtins/src/lib.rs`: Baseline global object installation.

**Testing:**
- `crates/test-harness/tests/test262_lite.rs`: Integration test for test262-lite suite.
- `crates/test-harness/fixtures/test262-lite/pass/`: Positive JS fixtures.
- `crates/test-harness/fixtures/test262-lite/fail/`: Negative parse/runtime fixtures.
- `crates/*/src/lib.rs`: Many crate-local unit tests are embedded in module files.

**Documentation:**
- `AGENTS.md`: Project execution constraints and phase context.
- `docs/current-status.md`: Current progress snapshot with phase and test baselines.
- `docs/test262-baseline.md`: Compatibility baseline tracking.

## Naming Conventions

**Files:**
- Crate roots use `src/lib.rs` (examples: `crates/vm/src/lib.rs`, `crates/parser/src/lib.rs`).
- Binary targets use `src/bin/*.rs` (example: `crates/test-harness/src/bin/test262-run.rs`).
- Documentation uses kebab-case markdown names (examples: `docs/current-status.md`, `docs/gc-test-plan.md`).
- Rust test files use snake_case with `_test` style directories or names (example: `crates/test-harness/tests/test262_lite.rs`).

**Directories:**
- Workspace crates use kebab-case (`crates/test-harness`, `crates/bytecode`).
- Fixture taxonomy uses semantic buckets (`crates/test-harness/fixtures/test262-lite/pass`, `crates/test-harness/fixtures/test262-lite/fail`).

**Special Patterns:**
- Internal engine markers use `"$__qjs_*__$"` constants in `crates/parser/src/lib.rs`, `crates/bytecode/src/lib.rs`, and `crates/vm/src/lib.rs`.
- Top-level planning docs are stored under `.planning/codebase/`.

## Where to Add New Code

**New JS Syntax/Semantics Feature:**
- Primary parser/lowering: `crates/parser/src/lib.rs` and `crates/bytecode/src/lib.rs`.
- Runtime behavior: `crates/vm/src/lib.rs`.
- Value model extensions: `crates/runtime/src/lib.rs` if new `JsValue`/native handles are required.
- Tests: crate-local unit tests plus harness coverage in `crates/test-harness/src/lib.rs`.

**New Built-in Global or Method:**
- Global registration: `crates/builtins/src/lib.rs`.
- Native enum surface: `crates/runtime/src/lib.rs` (`NativeFunction`).
- Implementation/dispatch: `crates/vm/src/lib.rs` (`execute_native_call` / host dispatch paths).
- Conformance tests: `crates/test-harness/fixtures/test262-lite/` and `crates/test-harness/tests/test262_lite.rs`.

**New Harness/CLI Capability:**
- Harness API/data structures: `crates/test-harness/src/test262.rs`.
- CLI flags/reporting: `crates/test-harness/src/bin/test262-run.rs`.
- CI usage: `.github/workflows/ci.yml`.

## Special Directories

**`target/`:**
- Purpose: Rust build output and local run artifacts.
- Source: Auto-generated by Cargo commands and local suite runs.
- Committed: No, ignored by `.gitignore`.

**`.planning/codebase/`:**
- Purpose: Maintained codebase mapping docs for orchestration and planning.
- Source: Written by mapper roles and updated over time.
- Committed: Yes, intended as project metadata.

**`.serena/`:**
- Purpose: Local tool cache/memory metadata.
- Source: Generated by tooling.
- Committed: Present in workspace; treat as tooling state, not engine source.

---

*Structure analysis: 2026-02-25*
*Update when directory layout or key entry points change*
