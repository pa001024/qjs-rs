# QuickJS to qjs-rs Mapping

| QuickJS area | qjs-rs crate | Status | Notes |
| --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | Arithmetic/unary/comparison, `===/!==`, `&&/||`, delimiters, block/call/member/bracket syntax tokens, line/block comment skipping, and basic string literal lexing landed; control-flow keywords are handled as identifiers in parser stage. |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | Script/block/function statements, `let/const/var`（含逗号声明）, `return`, `if/while/do-while/for/switch`, `break/continue`, `throw`, `try/catch/finally`, label/empty statement, unary/comparison (`===/!==`), logical ops (`&&/||`), call/assignment expressions, object/array literals, member access/update（含 `obj[key]`）, and `true/false/null/string` literals landed. |
| Bytecode compiler | `crates/bytecode` | In Progress | Script compilation, function table, call/return ops, scope ops, literal load ops (`bool/null/string`), object/array lowering（数组按对象+`length` 基线实现）, object create/get/set ops（含按值 key 访问）, logical short-circuit lowering, jump ops (`JumpIfFalse`/`Jump`), loop/switch control patching (`break/continue`), exception-handler ops (`throw`/`try`), and finally-aware abrupt completion emission landed. |
| VM execution | `crates/vm` | In Progress | Scope stack, lexical shadowing, hoisted function calls, jump-based control flow (`if/while/for/switch`), object property read/write（含 computed key）, `arguments` object baseline (`length` / indexed / `callee`), basic numeric coercion for arithmetic/comparison, string concatenation via `+`, exception unwinding (`throw`/`try`), realm fallback, and reference-based captures landed. |
| Value/object model | `crates/runtime` | In Progress | `JSValue::Object` landed with VM-side object storage; global identifier lookup via `Realm` and lexical env in VM scopes remain baseline. |
| Builtins | `crates/builtins` | Planned | Add core globals first, then advanced builtins. |
| Compatibility harness | `crates/test-harness` | In Progress | End-to-end expression/function/control-flow smoke tests, frontmatter-aware `test262-lite` runner, real test262 frontmatter extraction（支持版权头）, suite CLI (`test262-run`) with failure-sample output (`--show-failures`) landed; harness-global-dependent cases are temporarily skipped. |

## Immediate Next Slice
- Add array literals and more unary/control syntax (`delete`/`typeof`) to reduce parse failures in language/asi and arguments-object groups.
- Add strict mode + harness include plumbing, then wire minimal `assert`/`Test262Error` host side behavior.
- Extend object model with `this` baseline and property delete/enumeration basics.
