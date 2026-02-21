# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Arithmetic, unary, comparison, delimiters, block/call/member syntax tokens landed; control-flow keywords are handled as identifiers in parser stage. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Script/block/function statements, `let/const`, `return`, `if/while/for/switch`, `break/continue`, `throw`, `try/catch/finally`, unary/comparison, call/assignment expressions, object literals, and member access/update landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Script compilation, function table, call/return ops, scope ops, unary/comparison ops, object create/get/set ops, jump ops (`JumpIfFalse`/`Jump`), loop/switch control patching (`break/continue`), exception-handler ops (`throw`/`try`), and finally-aware abrupt completion emission landed. |
| VM execution | `crates/vm` | In Progress | Scope stack, lexical shadowing, hoisted function calls, jump-based control flow (`if/while/for/switch`), object property read/write, baseline exception unwinding (`throw`/`try`), realm fallback, and reference-based captures landed. |
| Value/object model | `crates/runtime` | In Progress | `JSValue::Object` landed with VM-side object storage; global identifier lookup via `Realm` and lexical env in VM scopes remain baseline. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression/function/control-flow smoke tests, frontmatter-aware `test262-lite` runner, and suite CLI (`test262-run`) landed. |

## Immediate Next Slice
- Expand from `test262-lite` to real test262 frontmatter-aware runner and produce baseline pass-rate report.
- Add strict mode and harness include plumbing for broader test262 coverage.
- Extend object model with bracket access, property delete/enumeration basics, and GC-aware layout planning.
