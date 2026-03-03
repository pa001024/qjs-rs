---
quick_task: 2
title: ts类型抹除回归收口
mode: quick
status: completed
created_at: 2026-03-04T00:30:00Z
---

# Quick Plan 2: ts类型抹除回归收口

## Task 1
- files: `crates/vm/src/lib.rs`, `crates/vm/src/fast_path.rs`
- action: 补齐 packet-i revalidate 的 VM 开关 API，并将 packet-d/packet-g revalidate 逻辑接入 packet-i 的 shadow-aware 校验路径，清理未使用 fast-path 死代码。
- verify: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`
- done: packet-i API 与重验证路径生效，相关 VM 回归通过。

## Task 2
- files: `crates/vm/tests/perf_packet_d.rs`
- action: 修复 clippy `type_complexity`，为复杂返回类型引入别名，保持测试语义不变。
- verify: `cargo clippy --workspace --all-targets -- -D warnings`
- done: clippy 全绿，无新增 lint 豁免。
