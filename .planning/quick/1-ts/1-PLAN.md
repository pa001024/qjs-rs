---
quick_task: 1
title: ts类型抹除
mode: quick
status: completed
created_at: 2026-03-03T20:15:56Z
---

# Quick Plan 1: ts类型抹除

## Task 1
- files: `crates/parser/src/lib.rs`
- action: 在 parser 增加 TypeScript 类型抹除路径，覆盖 `type/interface/declare` 声明吞吐、参数/返回值类型注解与 `as` 断言抹除，并补测试。
- verify: `cargo test -p parser`
- done: parser 单元测试和 module baseline 全部通过。

## Task 2
- files: `crates/parser/src/lib.rs`
- action: 进行工作区级回归并记录结果（fmt/clippy/test）。
- verify: `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`
- done: fmt 通过；clippy/test 失败点已记录（既有基线问题）。
