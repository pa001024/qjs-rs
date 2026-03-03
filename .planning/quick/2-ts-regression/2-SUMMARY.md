---
quick_task: 2
title: ts类型抹除回归收口
status: completed
executor_commit: 7c71944
date: 2026-03-04
---

# Quick Summary 2: ts类型抹除回归收口

## Delivered
- 在 `crates/vm/src/lib.rs` 增加 packet-i revalidate 开关 API：
  - `set_packet_i_revalidate_enabled`
  - `packet_i_revalidate_enabled`
- 将 packet-d / packet-g revalidate 逻辑扩展为 packet-i 可选的 shadow-aware 命中路径，保持关闭 packet-i 时的既有行为。
- 在 `crates/vm/src/fast_path.rs` 删除未使用的 scope 清理函数，消除 dead_code。
- 在 `crates/vm/tests/perf_packet_d.rs` 引入返回类型别名，修复 `clippy::type_complexity`。

## Regression
- `cargo fmt --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings`: pass
- `cargo test --workspace`: pass

## Commits
- `7c71944` feat-vm-packet-i-revalidate-regression-closure
