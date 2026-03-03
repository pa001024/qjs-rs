# Quick Task 4 Plan

## Goal
为 qjs-rs 增强对接 dna-builder 的宿主能力，补齐 Host Class、class async method、ScriptRuntime 可中断、console logger 注入与 Boa-like 兼容层，并提供回归测试与最小迁移示例。

## Task 1: Host Class 与 Async Method 能力
- files:
  - `crates/vm/src/host_adapter.rs` (new)
  - `crates/vm/src/lib.rs`
  - `crates/vm/src/script_runtime.rs`
- action:
  - 新增 Host Class 注册结构与 API（构造器/实例方法/静态方法，含 async 版本）。
  - 复用 opaque_data 完成 Rust 实例绑定与方法取回。
  - 确保构造器 new-only 约束与 prototype/constructor 语义可迁移。
- verify:
  - `cargo test -p vm --test host_adapter -- host_class_sync_methods --nocapture`
  - `cargo test -p vm --test host_adapter -- host_class_async_methods --nocapture`
- done:
  - Host class API 可注册并被 JS 侧 `new` + method 调用，async method 返回 Promise 并正确 settle。

## Task 2: ScriptRuntime Interrupt + Console Logger 注入
- files:
  - `crates/vm/src/script_runtime.rs`
  - `crates/vm/src/host_adapter.rs`
  - `crates/vm/tests/host_adapter.rs`
- action:
  - 为 ScriptRuntime 增加 stop token / interrupt 集成与 stop helper。
  - 增加 console log/info/warn/error/debug 可替换 logger 注入入口。
  - 验证中断时 pending class async promise reject 且无残留。
- verify:
  - `cargo test -p vm --test host_adapter -- interrupt_rejects_pending_class_async_promises --nocapture`
  - `cargo test -p vm --test host_adapter -- console_logger_bridge --nocapture`
- done:
  - 中断行为与 async host callback 语义一致，console 消息可桥接到宿主 logger。

## Task 3: Boa-like Adapter 与兼容回归
- files:
  - `crates/vm/src/host_adapter.rs`
  - `crates/vm/tests/host_adapter.rs`
  - `crates/vm/src/lib.rs`
- action:
  - 提供 Boa-like adapter：
    - `register_global_function`
    - `register_global_async_function`
    - `register_host_class`
    - `run_script_file` / `run_script_source`
    - `drain_jobs` / `interrupt`
  - 增补 API 中文注释与最小迁移示例测试。
- verify:
  - `cargo test -p vm --test host_adapter -- boa_like_adapter_smoke --nocapture`
  - `cargo test -p vm --test async_host_callbacks -- --nocapture`
  - `cargo test -p vm script_runtime -- --nocapture`
  - `cargo test -p test-harness --test rust_host_bindings -- --nocapture`
- done:
  - 新 API 满足 dna-builder 迁移能力集合，关键存量测试不回归。
