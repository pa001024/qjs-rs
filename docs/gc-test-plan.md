# GC Test Plan (Phase 4 Step 4)

基线日期：2026-02-23  
依赖输入：`docs/gc-design.md`, `docs/semantics-checklist.md`, `docs/root-strategy.md`

## 1. 目标

验证 GC 引入后不破坏现有语义，并可检测误回收/漏回收。

## 2. 测试矩阵

| 场景 | 正向用例 | 边界/异常用例 | 验收要点 |
| --- | --- | --- | --- |
| 闭包捕获 | 捕获对象在多次调用后仍可读写 | 释放外部引用后对象应可回收 | `ClosureRoot` 标记正确；无悬挂引用 |
| 异常传播 | `try/catch/finally` 路径对象不丢失 | 抛出后 unwind 完成对象可回收 | `VmExceptionRoot` 生命周期正确 |
| `with` 环境 | `with` 内属性访问正常 | 离开 `with` 后临时对象可回收 | `with_objects` root 进入/退出正确 |
| 原型链 | 原型属性读取/写入语义不变 | 深链对象释放后无残留引用 | `prototype` 边遍历完整 |
| 数组/对象混合 | 嵌套数组对象结构行为不变 | 批量分配后释放，回收统计上升 | 无明显泄漏，`sweep` 生效 |
| 句柄安全（generation） | 回收后重分配对象可正常读写 | 使用 stale `ObjectId` 访问时报 `UnknownObject` | slot 复用不导致陈旧句柄误命中 |
| host function pin（预留） | 已 pin 对象不被回收 | unpin 后对象可回收 | `HostFunctionRoot/HostPinRoot` 规则成立 |

## 3. 测试分层

1. 单元测试（VM/GC 内部）
- root 收集正确性（各 root 类是否进入快照）。
- mark 可达性（属性/原型/闭包/host 路径）。
- sweep 正确性（仅回收未标记对象）。

2. 集成测试（test-harness）
- 在 `run_script`/`run_expression` 路径加入 GC 触发点。
- 每类语义至少 1 个正向 + 1 个边界用例。

3. 兼容回归（test262-lite）
- 保持现有基线不回退。
- 新增 GC 相关 fixture（命名建议：`gc-*`）。

## 4. 执行命令

```powershell
cargo test -q
cargo test -p test-harness test262_lite
cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --show-failures 50
cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline
```

## 5. 验收标准

1. 全量回归不退化：`cargo test -q` 全绿。
2. GC 专项用例全部通过。
3. `gc_stats` 指标符合预期：
- 在“显式释放引用”样例中应观测到 `reclaimed_objects > 0`（当前 stress 快照已观测到 `reclaimed_objects=611`）。
- 稳态回归中 `remaining_objects` 不异常持续增长。
4. generation 回归成立：
- `crates/vm/src/lib.rs` 中 stale handle 用例稳定通过（回收+复用后旧句柄访问失败，新句柄成功）。
5. 每个新增语义点包含：
- 至少 1 个正向用例。
- 至少 1 个边界/异常用例。

## 6. 失败分级

| 等级 | 判定 | 处理 |
| --- | --- | --- |
| P0 | 崩溃、未定义行为、严重语义错误 | 阻断合并，立即修复 |
| P1 | 误回收/漏回收导致语义回归 | 进入当前迭代修复 |
| P2 | 统计偏差或非关键性能抖动 | 记录并在下一迭代处理 |

## 7. Step 4 验收记录

- 已覆盖闭包、异常、with、原型链、数组/对象混合、generation 句柄安全、host pin 预留七类高风险路径。
- 已给出可执行命令、验收口径与失败分级。
- 可直接进入 Step 5 PoC 实现与测试落地。

## 8. 集成进度（2026-02-23）

- 已新增 `test262-lite` GC 基础样例：
  - `crates/test-harness/fixtures/test262-lite/pass/gc-closure-capture.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-try-catch-finally.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-with-scope.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-nested-closure-chain.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-cycle-drop.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-looped-closures.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-deep-object-chain.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-array-object-churn.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-exception-closure-interleave.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-closure-chain-accumulator.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-looped-record-frame.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-cycle-triangle.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-array-ring-buffer.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-exception-nested-finally.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-closure-bucket-rotation.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-linked-list-rewrite.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-runtime-array-burst.js`
  - `crates/test-harness/fixtures/test262-lite/pass/gc-linked-stack-rotation.js`
- `test262-lite` 集成测试已启用自动 GC 压测模式（`auto_gc=true`, `threshold=1`）。
- 已支持运行中安全点压测参数（`runtime_gc=true`, `runtime_gc_check_interval=1`）。
- VM 已新增 generation 句柄安全单测：回收后 slot 复用会提升 generation，旧句柄访问返回 `UnknownObject`。
- VM 已新增 caller stack roots 回归：`runtime_gc_keeps_caller_stack_roots_across_nested_calls`，覆盖 array churn + nested call 场景。

## 9. Step 9 触发策略与观测校验

- Default Profile 验证：
  - `auto_gc=false` 且 `runtime_gc=false` 时 `gc_stats == GcStats::default()`。
  - `test262-run --show-gc`（默认配置）快照应为 `collections_total=0`、`boundary_collections=0`、`runtime_collections=0`。
  - 开启 `auto_gc` 且关闭 `runtime_gc` 时满足 `boundary_collections == collections_total` 且 `runtime_collections == 0`。
- Stress Profile 验证：
  - 执行 `test262-lite --auto-gc --auto-gc-threshold=1 --runtime-gc --runtime-gc-interval=1 --show-gc`，要求跑批通过（当前 26/26）。
  - Guard 示例：`cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline`。
  - 如需临时收紧阈值，可在 baseline 之上叠加 `--expect-*` 参数（显式参数优先）。
  - CLI guard 内置统计平衡校验：`collections_total == runtime_collections + boundary_collections`。
  - VM 统计满足 `runtime_collections > 0` 且 `collections_total == runtime_collections + boundary_collections`。
  - 预期关系：guard 命令成功时，至少满足 `collections_total >= 1000`、`runtime_collections >= 1000`、`runtime_collections / collections_total >= 0.90`、`reclaimed_objects >= 1`；并继续满足 `collections_total == runtime_collections + boundary_collections`。
  - 集成守护：`crates/test-harness/tests/test262_lite.rs` 已内置 `reclaimed_objects > 0` 与 runtime ratio `>= 0.9` 断言。
  - 当前快照：`collections_total=29283`、`boundary_collections=22`、`runtime_collections=29261`、`reclaimed_objects=611`。
- 观测要求：`gc_stats` 的 `objects_before/remaining/peak`、`mark_duration_ns`、`sweep_duration_ns` 等字段须与测试结果一并提交，便于追踪 default/stress 两套 profile 下的行为差异。
