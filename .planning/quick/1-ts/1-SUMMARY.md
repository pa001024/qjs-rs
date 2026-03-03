---
quick_task: 1
title: ts类型抹除
status: completed
executor_commit: af12670
date: 2026-03-03
---

# Quick Summary 1: ts类型抹除

## Delivered
- 在 `crates/parser/src/lib.rs` 增加 TS 语法抹除能力：
  - 语句级：`type` / `interface` / `declare` 声明吞吐。
  - 注解级：变量、函数参数、返回值类型注解抹除。
  - 表达式级：`as` 类型断言抹除。
  - module 级：跳过 `import type` / `export type` / `export interface` / `export declare` 行。
- 新增并通过 parser 回归测试，覆盖 typed function/arrow、as assertion、type-only declaration/import。

## Regression
- `cargo fmt --check`: pass
- `cargo test -p parser`: pass
- `cargo clippy --workspace --all-targets -- -D warnings`: fail（现有基线问题）
  - `crates/vm/src/lib.rs:897` `IdentifierFastPathFlags` dead_code
  - `crates/vm/src/fast_path.rs:301` dead_code
  - `crates/vm/src/fast_path.rs:472` dead_code
- `cargo test --workspace`: fail（现有基线问题）
  - `crates/vm/tests/perf_hotspot_attribution.rs:27` 缺少 `set_packet_i_revalidate_enabled`
  - `crates/vm/tests/perf_packet_d.rs:40` 缺少 `set_packet_i_revalidate_enabled`

## Commits
- `af12670` feat-parser-ts-type-erasure
