# Memory Inventory (Phase 4 Step 1)

基线日期：2026-02-23  
来源：并行子 agent 盘点（VM / runtime+builtins / harness）

## 1. 盘点范围

- VM 对象创建、持有与传播路径。
- Runtime `Realm` 与 builtins 注入后的全局持有关系。
- Test harness 执行入口对 `Vm`/`Realm` 生命周期的影响。

## 2. 对象与容器清单

| 对象/值类型 | 主要容器 | 当前 root 来源 | 回收风险 | 建议 |
| --- | --- | --- | --- | --- |
| `JsObject`（普通对象、数组、函数对象） | `Vm.objects` | 执行期间由 VM 对象表强持有 | 当前无 sweep，执行周期内对象只增不减 | 将 `Vm.objects` 改为“可标记后清扫”容器，按 roots 可达性回收。 |
| 变量绑定值（`JsValue`） | `Vm.bindings` + `Vm.scopes` | 词法环境与作用域栈 | 绑定长期存活会间接固定对象图 | GC 标记阶段按 `scopes -> bindings -> value` 逐级遍历。 |
| 操作数与中间值 | `Vm.stack` | 当前执行栈 | 深调用/异常路径易形成临时高峰 | 将 `stack` 作为一级 root，支持调试统计峰值。 |
| 异常相关值 | `Vm.exception_handlers`, `Vm.pending_exception` | handler 栈与 pending 异常槽 | unwind 不完整会遗留引用 | 明确 handler 进入/退出不变量；GC 必须标记 pending 异常值。 |
| `with` 与引用跟踪值 | `Vm.with_objects`, `Vm.identifier_references` | `with` 环境与引用缓冲 | 错误路径可能延迟释放引用 | 在 unwind 与函数返回路径统一清理，并纳入 root 标记。 |
| 闭包捕获 | `Vm.closures`, `Vm.closure_objects` | 闭包环境快照与函数对象关联 | 捕获链可跨多层作用域，易形成循环引用 | 闭包捕获对象与其环境必须整体标记，避免悬挂引用。 |
| 宿主函数桥接值 | `Vm.host_functions` | HostFunction 表 | host pin 未定义前可能误回收/误保活 | 引入显式 host pin 规则（见 `docs/root-strategy.md`）。 |
| 全局绑定与内建 | `Realm.globals` | Realm 全局表（`install_baseline` 注入） | 若 root 规则不清，可能出现 builtin 失效 | 将 `Realm.globals` 视为永久 root 区（直到 realm drop）。 |
| 原型与全局对象标识 | VM 内部原型/全局对象 ID 缓存 | 执行初始化阶段建立 | 若缓存失配会导致属性解析异常 | 原型 ID 与 `global_object_id` 归类为“执行上下文永久 root”。 |

## 3. 生命周期边界（当前实现）

1. 每次执行入口会重建 `Vm` 运行态（清理对象/闭包/handler 等容器并重新初始化）。
2. `Realm` 默认由调用方持有，在执行期间作为全局根集合存在。
3. test-harness 的 `run_expression`/`run_script` 与 test262 case 运行均采用“每次新建 `Vm` + `Realm`”策略，天然隔离单次执行的对象图。

## 4. 必须纳入首版 root set 的类别（Step 1 结论）

1. 执行栈与调用帧：`stack`、函数调用保存状态。
2. 作用域与绑定：`scopes`、`bindings`、`var_scope_stack`。
3. 全局与内建：`Realm.globals`、全局对象与原型缓存。
4. 异常与控制流：`exception_handlers`、`pending_exception`。
5. 闭包与宿主桥：`closures`、`closure_objects`、`host_functions`。

## 5. Step 1 验收记录

- 已覆盖长期任务要求的五类来源：VM 栈、全局对象、闭包环境、异常栈、内建缓存。
- 已给出每类对象的 root 来源与回收风险，可直接进入 Step 2 root 策略设计。
