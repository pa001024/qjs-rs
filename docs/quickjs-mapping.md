# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Number, identifier, `+ - * /`, `(`, `)` and EOF tokens landed. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Script statements, block statements, `let/const`, assignment, and precedence landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Script compilation with block scope entry/exit and variable ops landed. |
| VM execution | `crates/vm` | In Progress | Scope stack, shadowing, mutable/immutable binding checks, and realm fallback landed. |
| Value/object model | `crates/runtime` | In Progress | Global identifier lookup via `Realm` landed; lexical env currently lives in VM scopes. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression smoke tests landed. |

## Immediate Next Slice
- Add unary operators and comparison operators.
- Add function scope and call frame environments.
- Add module-style integration tests and golden parser tests.
