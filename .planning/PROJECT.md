# qjs-rs

## What This Is

`qjs-rs` is a pure-Rust JavaScript runtime library targeting QuickJS-aligned semantics with a Rust-native architecture.

## Core Value

Deliver QuickJS-aligned JavaScript semantics in a pure Rust runtime without introducing C FFI into the runtime core.

## Current State (after v1.0)

- Milestone `v1.0` shipped on **2026-02-27**
- Phase completion: **9/9**
- Plan completion: **26/26**
- Requirement coverage (v1): **20/20**
- Verification traceability is now machine-enforced through repo-local checker + CI gate

## Next Milestone Goals

- Define next milestone requirements (`$gsd-new-milestone`)
- Expand post-v1 language/runtime scope (e.g., Symbol/BigInt/Proxy/typed-array breadth)
- Continue compatibility and performance convergence while preserving semantic-first constraints

## Requirements

### Validated

- ✓ v1 semantic/runtime/builtins/module/async/governance closure shipped in v1.0
- ✓ Verification traceability normalization and CI enforcement shipped in v1.0

### Active

- [ ] Define vNext requirement set and acceptance criteria
- [ ] Prioritize post-v1 expansion areas (language breadth vs performance roadmap)

### Out of Scope

- Runtime-core C FFI integration
- CLI-first productization ahead of library/runtime priorities

## Context

Primary shipped artifacts are archived under `.planning/milestones/`:

- `v1.0-ROADMAP.md`
- `v1.0-REQUIREMENTS.md`
- `v1.0-MILESTONE-AUDIT.md`

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Semantic correctness > maintainability > performance | Keep behavior contracts stable before optimization | ✓ Good |
| Layered architecture (`parser -> bytecode -> vm -> runtime -> builtins`) | Limits regression blast radius and keeps ownership clear | ✓ Good |
| Verification schema normalization + CI traceability gate | Eliminates manual fallback and makes requirement coverage reproducible | ✓ Good |

---
*Last updated: 2026-02-27 after v1.0 milestone completion*
