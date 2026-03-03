# CRATES KNOWLEDGE BASE

**Scope:** `crates/` first-party Rust workspace members.

## OVERVIEW
`crates/` contains the primary engine implementation; this is the authoritative code path for parser, bytecode, runtime, VM, builtins, harness, and benchmarks.

## STRUCTURE
```text
crates/
├── ast/           # shared AST model
├── lexer/         # tokenization + literal scanning
├── parser/        # syntax + early errors + lowering
├── bytecode/      # opcode/chunk compiler
├── runtime/       # JsValue/Realm/native enums
├── builtins/      # baseline global installation
├── vm/            # execution core, module/gc/promise semantics
├── test-harness/  # test262-lite driver + CLI
└── benchmarks/    # contract-driven engine comparison runner
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Parse script/module behavior | `crates/parser/src/lib.rs` | strict-mode and early-error hotspot |
| Bytecode emission/contracts | `crates/bytecode/src/lib.rs` | opcode + identifier-slot metadata |
| Runtime value model | `crates/runtime/src/lib.rs` | `JsValue`, `NativeFunction`, `Realm` |
| VM execution and lifecycle | `crates/vm/src/lib.rs` | largest behavior surface |
| test262 suite orchestration | `crates/test-harness/src/test262.rs` | suite discovery + summary + gc stats |
| Benchmark schema + run profiles | `crates/benchmarks/src/contract.rs` | `bench.v1` constants and policy IDs |

## CONVENTIONS (CRATES)
- Crates and many tests use `#![forbid(unsafe_code)]` as a hard safety baseline.
- Integration tests are scenario-named (`*_baseline`, `*_semantics`, `perf_packet_*`) under each crate's `tests/`.
- Cross-crate execution tests should run through real compile+execute flows, not ad-hoc mocks.
- `test-harness` fixture updates are compatibility changes and must be treated as behavior-impacting.

## ANTI-PATTERNS (CRATES)
- Adding runtime-core C FFI dependencies.
- Bypassing parser strict-mode guards for forbidden bindings (`eval`, `arguments`, strict reserved names).
- Introducing benchmark changes in runtime crates without updating contract/runbook evidence.
- Treating fixture churn as "test-only" when it changes language behavior expectations.

## COMMANDS
```bash
cargo test -p parser
cargo test -p vm
cargo test -p test-harness --test test262_lite

cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite
```

## NOTES
- Prefer crate-local AGENTS docs (`crates/vm/AGENTS.md`, `crates/test-harness/AGENTS.md`) for subsystem details.
- `crates/benchmarks` is policy-coupled to docs + CI scripts; do not treat it as isolated tooling.
