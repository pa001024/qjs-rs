# qjs-rs

## What This Is

`qjs-rs` is a pure-Rust JavaScript runtime library that targets QuickJS-aligned semantics while keeping a maintainable Rust-native architecture.  
It already runs an end-to-end pipeline (`parser -> bytecode -> vm -> runtime -> builtins -> test-harness`) and is validated by `cargo test` plus test262 subsets.  
The project is for engine/runtime maintainers who need spec-correct behavior first, then performance hardening.

## Core Value

Deliver QuickJS-aligned JavaScript semantics in a pure Rust runtime without introducing C FFI into the runtime core.

## Requirements

### Validated

- ✓ Workspace + CI baseline is operational (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) — existing
- ✓ Script execution pipeline is runnable end-to-end through `crates/test-harness` — existing
- ✓ test262 harness and regression workflow are integrated with repeatable snapshots — existing
- ✓ Core crates are in place and connected (`crates/{lexer,parser,ast,bytecode,vm,runtime,builtins,test-harness}`) — existing

### Active

- [ ] Complete remaining language-core semantic gaps for object model, descriptor edges, and eval/with/strict interactions
- [ ] Harden GC behavior and root strategy under stress/performance scenarios
- [ ] Expand builtins from baseline/minimal support to broader ECMAScript coverage
- [ ] Implement ES Module lifecycle (parse/instantiate/evaluate) and Promise job queue semantics
- [ ] Increase compatibility breadth and stability in larger test262 subsets and nightly runs

### Out of Scope

- Runtime-core C FFI integration — forbidden by project boundary
- Premature micro-optimizations that trade away semantic correctness — correctness is prioritized over speed
- CLI-first productization — delivery target is library-first; CLI shell is optional later

## Context

This codebase is brownfield with substantial progress already landed.  
The current strategy is semantic alignment with QuickJS behavior models, guided by Rust-friendly architecture patterns influenced by Boa.  
Primary references and planning anchors are:

- `AGENTS.md`
- `docs/current-status.md`
- `docs/quickjs-mapping.md`
- `docs/semantics-checklist.md`
- `docs/risk-register.md`
- `.planning/codebase/ARCHITECTURE.md`
- `.planning/codebase/CONCERNS.md`

## Constraints

- **Architecture**: Pure Rust runtime core, no C FFI — maintain portability and ownership clarity
- **Prioritization**: Semantic correctness > maintainability > performance — prevents premature optimization drift
- **Delivery**: Library-first API surface — engine capabilities must be consumable by Rust callers
- **Compatibility**: Keep QuickJS semantic direction while allowing Rust-native internal implementation — alignment without mechanical translation
- **Quality Gate**: CI must keep `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` green — protects iterative delivery

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Align semantics with QuickJS behavior, not source-level translation | C implementation details do not map cleanly to Rust ownership/lifetimes | ✓ Good |
| Keep workspace layering `parser -> bytecode -> vm -> runtime -> builtins` | Clear module boundaries reduce semantic regression blast radius | ✓ Good |
| Use phased delivery with milestone gates | Prevents scope explosion and enables incremental verification | ✓ Good |
| Continue roadmap from current brownfield status rather than restarting from scaffold assumptions | Existing implementation already passed key baseline gates | ✓ Good |

---
*Last updated: 2026-02-25 after new-project initialization*
