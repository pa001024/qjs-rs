# GC Design (Phase 4 Step 3)

基线日期：2026-02-23  
依赖输入：`docs/memory-inventory.md`, `docs/root-strategy.md`

## 1. 设计目标

- 首版实现可验证正确性的 mark-sweep。
- 不改变现有 `JsValue::Object(ObjectId)` 外部语义。
- 优先保证语义稳定，再考虑性能。

## 2. 对象图与根集

首版标记入口来自 `collect_roots()`，覆盖：

1. 执行根：`stack`、`scopes`、`bindings`、`var_scope_stack`。
2. 异常根：`exception_handlers`、`pending_exception`。
3. 对象表根：`objects`、全局对象/原型 ID 缓存。
4. 闭包根：`closures`、`closure_objects`。
5. 宿主根：`host_functions` + `HostPinRoot`（pin/unpin 表）。
6. 全局根：`Realm.globals`（含 baseline builtins）。

## 3. 标记算法（Mark）

### 3.1 标记规则

- 灰栈迭代（非递归）避免深图递归栈溢出。
- 遇到 `JsValue::Object(id)`：
  - 标记对象本身。
  - 遍历 `properties/getters/setters` 中的值。
  - 跟踪 `prototype` 边。
- 遇到 `JsValue::Function(closure_id)`：
  - 标记闭包对象。
  - 遍历 `captured_scopes -> bindings -> value`。
- 遇到 `JsValue::HostFunction(host_id)`：
  - 按 host function 负载继续追踪 `target/this_arg/bound_args/object_id` 等引用。

### 3.2 伪代码

```rust
fn gc_collect(vm: &mut Vm, realm: &Realm) -> GcStats {
    let roots = collect_roots(vm, realm);
    mark_from_roots(vm, &roots);
    sweep_unreachable(vm)
}

fn mark_from_roots(vm: &mut Vm, roots: &[JsValue]) {
    vm.gc_mark_stack.clear();
    for v in roots {
        schedule(vm, v.clone());
    }
    while let Some(v) = vm.gc_mark_stack.pop() {
        match v {
            JsValue::Object(id) => mark_object(vm, id),
            JsValue::Function(fid) => mark_closure(vm, fid),
            JsValue::HostFunction(hid) => mark_host_function(vm, hid),
            _ => {}
        }
    }
}
```

## 4. 清扫算法（Sweep）与 ObjectId 代际策略

### 4.1 Sweep

- 遍历 `Vm.objects`（键为完整 `ObjectId` 句柄），回收未标记对象。
- 对回收对象执行：
  - 从对象表删除；
  - 记录统计（`reclaimed/remaining`）；
  - 仅回收 `slot` 到 `free_object_slots`，供后续分配复用。

### 4.2 ObjectId 编码与复用（已落地）

- `ObjectId` 仍保持 `u64`，编码为：`(generation << 32) | slot`。
- 新增分配辅助：
  - `allocate_object_id()`：优先复用 `free_object_slots`；
  - 复用 slot 时强制 `generation + 1`；
  - 新 slot 使用 `generation=0`。
- 结果：同一 slot 再分配时 `ObjectId` 必变，旧句柄即使数值 slot 相同也不会命中新对象，避免 stale handle 误命中。

## 5. 当前数据结构（已落地）

1. `Vm` 对象分配字段：
  - `next_object_slot: u32`
  - `object_generations: Vec<u32>`
  - `free_object_slots: Vec<u32>`
2. GC/观测字段：
  - `gc_mark_stack: Vec<JsValue>`
  - `gc_shadow_roots: Vec<GcShadowRoots>`（用于运行中 GC 期间保留调用方上下文根）
  - `gc_last_stats: GcStats`
  - `gc_peak_objects`, `gc_collections_total`, `gc_boundary_collections`, `gc_runtime_collections`
3. 触发控制：
  - `auto_gc_enabled`, `auto_gc_object_threshold`
  - `runtime_gc_enabled`, `runtime_gc_check_interval`
4. 宿主根：
  - `host_pins` + `pin_host_value/unpin_host_value`

## 6. 接口契约（Step 3 产出）

| 接口 | 说明 |
| --- | --- |
| `collect_roots(vm, realm) -> RootSnapshot` | 采集稳定 root 快照，不修改容器。 |
| `mark_from_roots(vm, roots)` | 标记可达对象图。 |
| `sweep_unreachable(vm) -> GcStats` | 清扫不可达对象并记录统计。 |
| `gc_stats(vm) -> GcStats` | 查询上次 GC 统计。 |
| `gc_trigger(vm) -> bool` | 基于阈值或显式请求判断是否触发 GC。 |

## 7. 触发策略（首版）

- 自动触发：对象数超过 `auto_gc_object_threshold`（且 `auto_gc_enabled=true`）。
- 手动触发：测试/调试路径显式调用。
- 安全降级：若发现语义回归，可临时关闭自动触发，仅保留执行边界触发。

当前实现状态（2026-02-23）：
- 已提供 `enable_auto_gc(bool)` 与 `set_auto_gc_object_threshold(usize)` 配置接口。
- 自动触发挂点位于 `execute_in_realm` 执行边界（成功返回后触发），默认关闭以保证兼容。
- 已提供运行中安全点触发开关：`enable_runtime_gc(bool)` 与 `set_runtime_gc_check_interval(usize)`。
- `gc_stats` 已扩展观测字段：`objects_before/remaining/peak` 与 `mark_duration_ns/sweep_duration_ns`。
- 已提供 `HostPinRoot` 最小 API：`pin_host_value` / `unpin_host_value`。
- 已修复嵌套调用下 caller stack roots 丢失问题：运行中 GC 会扫描 `gc_shadow_roots`，避免 `UnknownObject` 误回收。

### 7.1 Step 9 观测与触发 profile

- Default Profile：
  - `auto_gc_enabled=false`、`runtime_gc_enabled=false` 时不触发自动回收（`gc_stats == GcStats::default()`）。
  - 开启 `auto_gc` 且关闭 `runtime_gc` 时，统计应满足 `boundary_collections == collections_total` 且 `runtime_collections == 0`。
- Stress Profile：
  - `auto_gc_enabled=true`（threshold=1）+ `runtime_gc_enabled=true`（interval=1）。
  - 统计应满足 `runtime_collections > 0` 且 `collections_total == runtime_collections + boundary_collections`。
  - 在 `test262-lite --auto-gc --runtime-gc` 下维持全绿（当前 26/26）。

## 8. 风险与降级

1. 风险：根采集遗漏导致误回收。  
降级：先限制 GC 在执行边界触发，逐步放开到运行中触发。

2. 风险：ID 复用导致陈旧引用。  
降级：已采用 `ObjectId(slot+generation)`，复用 slot 时强制 generation 递增并在查找时按完整句柄匹配。

3. 风险：host 桥接对象生命周期不清。  
降级：在 `HostPinRoot` 完整治理前，仍将关键 host function 视为强根并加强回归约束。

## 9. Step 3 验收记录

- 已形成 mark/sweep 完整算法方案。
- 已明确接口契约与最小改造面。
- 可直接进入 Step 4（回归计划）与 Step 5（PoC 实现）。
