# Stack Research

**Domain:** Pure Rust JavaScript runtime library aligned with QuickJS semantics
**Researched:** 2026-02-25
**Confidence:** HIGH for repo-current decisions, MEDIUM for forward-looking additions

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust (stable toolchain) | >= 1.85, Edition 2024 | Runtime core (`parser -> bytecode -> vm -> runtime -> builtins`) | Already validated in repo CI and matches 2025 standard for safety-first systems work; keeps runtime core pure Rust and maintainable. (HIGH) |
| Cargo workspace (resolver = 2) | Current workspace layout | Modular engine development across crates | Current architecture already enforces clean boundaries; this is the standard way to evolve compiler/runtime projects incrementally without rewrites. (HIGH) |
| test262-driven conformance + QuickJS behavioral diffing | test262 subsets + snapshots (ongoing) | Semantic correctness gate | JS engine projects in 2025 still rely on conformance suites plus reference-behavior comparisons; current repo already uses this successfully. (HIGH) |
| GitHub Actions CI with strict quality gates | actions/checkout@v4, rust-toolchain@stable, rust-cache@v2 | Continuous verification (`fmt`, `clippy -D warnings`, `test`, GC guard) | Current pipeline is correct for brownfield evolution; preserves correctness-first priority and prevents silent semantic regressions. (HIGH) |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `regex` | 1.x | Fast regular-expression primitives for engine internals | Use for lexer/fast-path matching where ECMAScript backtracking features are not required. (HIGH) |
| `fancy-regex` | 0.14.x | Backtracking-capable regexp behavior for JS compatibility gaps | Keep for ECMAScript regexp semantics where `regex` alone is insufficient. (HIGH) |
| `proptest` (recommended add) | 1.x | Property-based tests for parser/VM invariants | Add during semantic edge-case hardening (`eval/with/descriptor/GC`) to find state-space bugs earlier. (MEDIUM) |
| `insta` (recommended add) | 1.x | Snapshot testing for parser/bytecode outputs | Add for stable AST/opcode snapshots when refactoring compiler stages. (MEDIUM) |
| `criterion` (recommended add) | 0.5.x | Microbenchmarking for Phase 7 performance work | Add once semantic closure is stable; keep perf checks separate from correctness CI gates. (MEDIUM) |
| `serde` + `serde_json` (recommended in harness/docs path) | 1.x | Structured baseline artifacts and telemetry output | Use for machine-readable compatibility/GC reports consumed by roadmap and regressions. (MEDIUM) |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo nextest` | Faster and more parallel test execution | Use for local/nightly acceleration; keep `cargo test` as mandatory compatibility gate in CI. |
| `cargo llvm-cov` | Coverage tracking for semantic blind spots | Useful when prioritizing next failure clusters from test262. |
| `cargo deny` | License and dependency policy checks | Fits pure-Rust/no-FFI governance and supply-chain hygiene. |
| `cargo audit` | Security advisory scanning for dependencies | Useful as nightly gate; avoid blocking fast inner-loop development. |
| `rustfmt` + `clippy` | Style and lint enforcement | Already required in current CI, keep unchanged. |

## Installation

```bash
# Toolchain
rustup toolchain install stable
rustup component add rustfmt clippy

# Recommended dev tools
cargo install cargo-nextest cargo-llvm-cov cargo-deny cargo-audit

# Recommended incremental crate additions
cargo add --dev proptest insta criterion
cargo add serde serde_json --package test-harness
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Pure Rust runtime core | C FFI runtime core (`quickjs-sys`, `pcre2-sys`, etc.) | Only for one-off differential tooling outside runtime core, never as production engine core in this project. |
| `regex` + `fancy-regex` hybrid | Single external regex engine via FFI | Consider only if a specific test262 blocker cannot be resolved in pure Rust, and keep it outside core runtime boundary. |
| `cargo test` + `cargo nextest` | Custom ad-hoc shell test orchestration | Use ad-hoc scripts only for temporary local triage; not as canonical regression pipeline. |
| `criterion` for perf phase | Early low-level profiling first | Use low-level profilers directly when a known hotspot is already identified and reproducibility is controlled. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Runtime-core C FFI dependencies | Violates explicit project boundary and increases portability/maintenance risk | Keep runtime core pure Rust; use reference engines only for behavior comparison. |
| Early NaN-boxing + heavy `unsafe` pointer tricks | Premature optimization increases semantic and GC bug risk in current milestone stage | Keep enum/handle model until compatibility targets are stable. |
| Embedding `tokio`/external async runtime into VM core | ECMAScript job queue semantics become host-runtime coupled and harder to reason about | Implement explicit engine-owned microtask queue in runtime layer (Phase 6). |
| Multi-thread shared mutable heap as default VM model | Adds synchronization and GC complexity before semantics are closed | Keep deterministic single-thread core; add concurrency only behind host boundaries later. |
| Replacing current parser pipeline with general-purpose frontend (e.g., SWC) | Risks semantic drift and expensive integration churn in brownfield | Continue incremental fixes in existing parser/bytecode stack aligned to QuickJS behavior. |

## Stack Patterns by Variant

**If milestone focus is semantic closure (current brownfield state):**
- Keep deterministic single-thread VM + strict CI + test262 subset expansion.
- Because correctness debugging needs reproducible execution order and controlled GC behavior.

**If milestone focus is GC and memory hardening:**
- Add property-based testing (`proptest`) and structured GC telemetry (`serde_json`) before major algorithm changes.
- Because visibility and invariant checks reduce regression risk in mark-sweep/root-management evolution.

**If milestone focus is performance convergence (later Phase 7):**
- Add `criterion` benchmarks and profile-guided optimization loop, while preserving semantic gates as release blockers.
- Because performance wins without conformance discipline create unstable regressions.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `rustc >=1.85` (Edition 2024) | `rustfmt` + `clippy` on stable | Matches current workspace and CI configuration. |
| `regex 1.x` | `fancy-regex 0.14.x` | Current VM stack already uses both; keep this split model for JS regexp behavior. |
| `cargo nextest` (current stable) | Cargo workspace (`resolver = 2`) | Drop-in test runner for faster feedback; does not replace canonical `cargo test` gate. |
| `criterion 0.5.x` | Stable Rust toolchain | Recommended for perf-only jobs, not required for every PR gate. |
| `serde_json 1.x` | test-harness result artifacts | Best used in harness/reporting path, not VM hot path. |

## Sources

- `.planning/PROJECT.md` - project constraints and brownfield direction (updated 2026-02-25)
- `.planning/codebase/STACK.md` - current in-repo stack and versions (analysis date 2026-02-25)
- `.planning/codebase/ARCHITECTURE.md` - current crate boundaries and data flow (analysis date 2026-02-25)
- `docs/current-status.md` - latest compatibility/GC progress snapshot (baseline 2026-02-25, updates through 2026-02-26)
- `Cargo.toml` - workspace edition/rust-version/member layout
- `crates/vm/Cargo.toml` - current regex library choices (`regex`, `fancy-regex`)
- `.github/workflows/ci.yml` - active CI gates and GC guard stress command

---
*Stack research for: pure Rust JavaScript runtime (QuickJS-aligned)*
*Researched: 2026-02-25*
