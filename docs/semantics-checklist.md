# Semantics Checklist

## Language Core
- [ ] Numeric literals and arithmetic evaluation order.
- [ ] Variable binding rules (`var`, `let`, `const`).
- [ ] Function declarations, closures, and `this` binding.
- [ ] Object property access and descriptor behavior.
- [ ] Prototype chain lookup and mutation.

## Control Flow and Errors
- [x] `if`, `while`, `for`.
- [x] `switch`.
- [ ] `try/catch/finally` propagation semantics (baseline landed; `finally` with `return`/`break`/`continue` pending).
- [x] `throw` behavior and stack/context handoff (baseline `throw`/catch/finally unwind landed).
- [x] `return`, `break`, `continue` jump semantics (baseline jump behavior landed).

## Runtime Model
- [ ] `JSValue` representation stability and correctness.
- [ ] Global/lexical environment separation.
- [ ] Object lifecycle and GC root correctness.

## Platform Features
- [ ] Promise microtask queue ordering.
- [ ] ES module resolution and instantiation order.
- [ ] Builtins baseline (`Object`, `Array`, `Function`, `Error`, `JSON`).
