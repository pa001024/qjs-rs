# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Number, identifier, `+`, `(`, `)` and EOF tokens landed. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Additive expressions and parenthesized parsing landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Literal/identifier loading and `Add` opcode landed. |
| VM execution | `crates/vm` | In Progress | Numeric `Add` execution and stack-underflow checks landed. |
| Value/object model | `crates/runtime` | Planned | Start with simple value enum, grow to object handles. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression smoke tests landed. |

## Immediate Next Slice
- Extend parser precedence table (multiplicative/comparison).
- Implement environment-backed identifier resolution in VM/runtime.
- Add module-style integration tests and golden parser tests.
