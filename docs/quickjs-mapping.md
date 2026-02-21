# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Arithmetic, unary, comparison, delimiters, block/call syntax tokens landed; control-flow keywords are handled as identifiers in parser stage. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Script/block/function statements, `let/const`, `return`, `if/while/for/switch`, `break/continue`, unary/comparison, and call/assignment expressions landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Script compilation, function table, call/return ops, scope ops, unary/comparison ops, jump ops (`JumpIfFalse`/`Jump`), loop/switch control patching (`break/continue`), bindings, and declaration hoisting landed. |
| VM execution | `crates/vm` | In Progress | Scope stack, lexical shadowing, hoisted function calls, unary/comparison execution, jump-based control flow (`if/while/for/switch`), realm fallback, and reference-based captures landed. |
| Value/object model | `crates/runtime` | In Progress | Global identifier lookup via `Realm` landed; lexical env currently lives in VM scopes. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression/function/control-flow smoke tests landed. |

## Immediate Next Slice
- Add exceptions (`throw`, `try/catch/finally`) with VM-level unwind support.
- Add function semantics hardening: hoisting edge behavior and strictness distinctions.
- Start object/property model (`JSValue::Object`, property lookup/update) as runtime baseline.
