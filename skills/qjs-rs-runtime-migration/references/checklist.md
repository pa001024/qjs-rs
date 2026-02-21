# Milestone Checklist

## Phase 0
- Workspace crates created and compile.
- Docs initialized:
  - `docs/quickjs-mapping.md`
  - `docs/semantics-checklist.md`
  - `docs/risk-register.md`
- CI baseline in place.

## Feature Slice Checklist
- Scope is explicit and small.
- Runtime behavior documented before coding.
- Tests include:
  - positive behavior
  - edge/error behavior
- `cargo fmt --check` passes.
- `cargo clippy -- -D warnings` passes.
- `cargo test` passes.

## Parity Checklist
- Compared against QuickJS behavior.
- Divergence logged with rationale.
- Follow-up tasks captured.
