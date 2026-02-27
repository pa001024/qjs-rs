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

## Current Milestone: v1.1 Performance Acceleration

**Goal:** Optimize runtime hot paths so the qjs-rs benchmark aggregate latency is at least on par with (or better than) `boa-engine` while preserving semantic correctness.

**Target features:**
- Add reproducible cross-engine performance baselines (`qjs-rs`, `boa-engine`, `quickjs-c`, `nodejs`) as first-class milestone evidence.
- Implement VM/runtime/bytecode hot-path optimizations for arithmetic loops, array workloads, and function-call-heavy cases.
- Gate performance regressions in CI with explicit thresholds and non-regression semantic checks.

## Next Milestone Goals

- Reach `qjs-rs <= boa-engine` aggregate mean latency on the local benchmark suite.
- Keep existing semantic and governance gates green while introducing performance changes.
- Build an optimization playbook that can be iterated toward QuickJS(C)-class performance.

## Requirements

### Validated

- ✓ v1 semantic/runtime/builtins/module/async/governance closure shipped in v1.0
- ✓ Verification traceability normalization and CI enforcement shipped in v1.0

### Active

- [ ] Define and approve milestone v1.1 performance requirements
- [ ] Execute v1.1 roadmap to beat boa-engine on the tracked benchmark suite without semantic regressions
- [ ] Add stable performance baselines and threshold gates for CI/regression tracking

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
*Last updated: 2026-02-27 after v1.1 milestone initialization*
