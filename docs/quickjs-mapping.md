# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Number, identifier, `+ - * /`, `(`, `)` and EOF tokens landed. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Script-level statements, `let/const`, assignment, and expression precedence landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Variable definition/store ops and script compilation landed. |
| VM execution | `crates/vm` | In Progress | Mutable/immutable bindings, assignment checks, and realm-backed lookup landed. |
| Value/object model | `crates/runtime` | In Progress | Global identifier lookup via `Realm` landed; local binding env lives in VM for now. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression smoke tests landed. |

## Immediate Next Slice
- Add unary operators and comparison operators.
- Add lexical scope nesting and block environments (currently single VM scope).
- Add module-style integration tests and golden parser tests.
