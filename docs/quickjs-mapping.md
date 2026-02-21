# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Number, identifier, arithmetic ops, delimiters, blocks, and call syntax tokens landed. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Script/block/function statements, `let/const`, `return`, call/assignment expressions landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Script compilation, function table, call/return ops, scope entry/exit, and bindings landed. |
| VM execution | `crates/vm` | In Progress | Scope stack, lexical shadowing, function closures/calls, mutable checks, and realm fallback landed. |
| Value/object model | `crates/runtime` | In Progress | Global identifier lookup via `Realm` landed; lexical env currently lives in VM scopes. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression smoke tests landed. |

## Immediate Next Slice
- Add unary operators and comparison operators.
- Align closure environment semantics with JS lexical-by-reference behavior.
- Add module-style integration tests and golden parser tests.
