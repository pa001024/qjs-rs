# Quick Task 4 Summary

## Task
你正在为 qjs-rs 增强对接 dna-builder 的宿主能力：补齐 Host Class、class async method、ScriptRuntime stop/interrupt、console logger 注入与 Boa-like 兼容层，并提供回归测试与迁移示例。

## Implemented

### 1) Host Class + Async Method
- 新增 `crates/vm/src/host_adapter.rs`：
  - `HostClassRegistration<T>`
  - `HostClassMethodRegistration<T>`
  - `HostClassAsyncMethodRegistration<T>`
  - `HostClassStaticMethodRegistration`
  - `HostClassAsyncStaticMethodRegistration`
- 在 `ScriptRuntime` 上新增：
  - `register_host_class`（构造器 + 实例/静态方法，支持 async）
- 构造器强制 new-only，实例通过 `opaque_data` 绑定，实例方法按 `this` 安全取回。

### 2) ScriptRuntime 中断能力
- `crates/vm/src/script_runtime.rs` 新增：
  - `set_stop_token`
  - `clear_stop_token`
  - `request_stop`
  - `drain_jobs`
- 中断行为对齐 VM：返回 `Interrupted`，并复用已有逻辑 reject pending async host promises。

### 3) Console 注入能力
- 新增 `ConsoleLevel`、`ConsoleLogger`。
- 在 `ScriptRuntime` 新增：
  - `inject_console_logger`
  - `inject_console_logger_shared`
- 注入 `console.log/info/warn/error/debug`，支持宿主桥接外部事件系统。

### 4) Boa-like 兼容层
- 新增 `BoaLikeHostAdapter`：
  - `register_global_function`
  - `register_global_async_function`
  - `register_host_class`
  - `run_script_source`
  - `run_script_file`
  - `drain_jobs`
  - `set_stop_token`
  - `interrupt`
  - `clear_interrupt`
  - `inject_console_logger`

### 5) 导出与可用性
- `crates/vm/src/lib.rs` 导出新增 adapter/class/console API。

## Tests
新增 `crates/vm/tests/host_adapter.rs`：
- `host_class_sync_methods`
- `host_class_async_methods`
- `interrupt_rejects_pending_class_async_promises`
- `console_logger_bridge`
- `boa_like_adapter_smoke`

并通过关键回归：
- `cargo test -p vm --test host_adapter -- --nocapture`
- `cargo test -p vm --test async_host_callbacks -- --nocapture`
- `cargo test -p vm script_runtime -- --nocapture`
- `cargo test -p test-harness --test rust_host_bindings -- --nocapture`
- `cargo fmt --check`

## Commit
- `dae7963` feat(vm): add boa-like host adapter class/console/interrupt APIs
