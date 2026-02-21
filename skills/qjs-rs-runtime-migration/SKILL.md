---
name: qjs-rs-runtime-migration
description: Plan and execute the pure Rust migration of QuickJS into qjs-rs with semantic parity checkpoints. Use when mapping QuickJS C modules to Rust crates, defining phased implementation milestones, creating runtime architecture decisions, or validating behavior against QuickJS, Boa, and test262 expectations.
---

# QJS RS Runtime Migration

## Overview

Build a repeatable workflow for porting QuickJS semantics into a pure Rust runtime.
Drive work in phases: architecture baseline, parser/bytecode/VM/runtime implementation, and compatibility validation.

## Workflow

1. Collect baseline inputs.
   - Read project `AGENTS.md`.
   - Inspect `D:\dev\QuickJS` for source layout and behavior entry points.
   - Inspect `D:\dev\boa` for Rust implementation patterns and tradeoffs.
2. Build and maintain semantic mapping.
   - Use `references/workflow.md` to map QuickJS modules into qjs-rs crate ownership.
   - Record unstable/high-risk semantics in `docs/risk-register.md`.
3. Execute one milestone at a time.
   - Scope feature slice tightly (e.g., parser core, closure capture, object property model).
   - Require tests in the same change set.
4. Validate continuously.
   - Run workspace checks (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`).
   - Run targeted compatibility checks and log deltas in docs.

## Deliverables Per Milestone

1. Working code path for the scoped feature.
2. Tests that cover happy path and edge/error behavior.
3. Updated mapping/checklist docs reflecting status and known gaps.

## Resources

1. `references/workflow.md`
   - Read when deciding crate boundaries, feature order, and parity strategy.
2. `references/checklist.md`
   - Read when preparing acceptance criteria for each milestone.
3. `scripts/inventory_sources.ps1`
   - Run when you need a quick inventory of QuickJS/Boa source files and top-level folders.
