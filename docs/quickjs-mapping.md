# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Number, identifier, `+ - * /`, `(`, `)` and EOF tokens landed. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Additive/multiplicative precedence and parenthesized parsing landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Literal/identifier loading and arithmetic opcodes landed. |
| VM execution | `crates/vm` | In Progress | Numeric `+ - * /` execution and realm-backed identifier resolution landed. |
| Value/object model | `crates/runtime` | In Progress | Global identifier lookup via `Realm` landed. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression smoke tests landed. |

## Immediate Next Slice
- Add unary operators and comparison operators.
- Add assignment and scoped bindings (`let`/`const`) on top of `Realm`.
- Add module-style integration tests and golden parser tests.
