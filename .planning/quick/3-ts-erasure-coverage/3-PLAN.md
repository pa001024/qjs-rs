---
quick_task: 3
title: ts类型抹除覆盖扩展
mode: quick
status: completed
created_at: 2026-03-04T02:45:00Z
---

# Quick Plan 3: ts类型抹除覆盖扩展

## Task 1
- files: `crates/parser/src/lib.rs`, `crates/lexer/src/lib.rs`
- action: 扩展 TS 抹除能力到 `satisfies` 链式、装饰器 token、`enum/namespace`、泛型参数和类修饰符语法通路。
- verify: `cargo test -p parser`; `cargo clippy --workspace --all-targets -- -D warnings`
- done: parser/lexer 相关新增与既有用例全部通过。

## Task 2
- files: `crates/parser/src/lib.rs`
- action: 扩展 module 解析期 type-only 抹除，覆盖 `import type {A}`、`import { type A }`、`export { A }`（type-only 来源）以及 `interface A {} + export { A }` 场景；补 `declare module/global` 用例。
- verify: `cargo test -p parser`; `cargo test --workspace`
- done: 新增覆盖场景均通过，全工作区回归通过。
