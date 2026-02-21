# Workflow Reference

## Goal
Map QuickJS behavior into a pure Rust architecture without C FFI.

## Mapping Method
1. Start from behavior, not source lines.
2. For each QuickJS area, define:
   - semantic contract
   - Rust owner crate
   - required tests
3. Track nontrivial behavior as explicit risks.

## Suggested Ownership
- `crates/lexer`: tokenization rules and trivia policy.
- `crates/parser`: AST construction and parse errors.
- `crates/ast`: syntax tree data structures.
- `crates/bytecode`: opcode design and compilation output.
- `crates/vm`: execution engine, call frames, control flow.
- `crates/runtime`: value model, objects, environments, GC hooks.
- `crates/builtins`: standard objects and host-exposed builtins.
- `crates/test-harness`: integration and compatibility runners.

## Execution Rhythm
1. Pick one slice.
2. Implement minimal complete path.
3. Add tests.
4. Document parity result and next gap.
