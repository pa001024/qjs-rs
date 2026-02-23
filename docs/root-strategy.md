# Root Strategy (Phase 4 Step 2)

基线日期：2026-02-23  
依赖输入：`docs/memory-inventory.md`, `docs/risk-register.md`

## 1. 目标

定义首版 GC root 集合边界、进入/退出条件、错误场景处理规则，为后续 mark-sweep 落地提供稳定契约。

## 2. Root 分类（v0）

| Root 类别 | 主要来源 | 进入条件 | 退出条件 | 错误场景 |
| --- | --- | --- | --- | --- |
| `RealmGlobalRoot` | `Realm.globals` + baseline builtins | 创建 `Realm` 并安装 baseline 后生效 | realm drop 或显式清空全局 | 全局未初始化导致 `UnknownIdentifier`。 |
| `VmExecutionRoot` | `Vm.stack`, `Vm.scopes`, `Vm.bindings`, `var_scope_stack` | `execute_in_realm` 开始执行 chunk | 执行完成后 VM 重置或帧弹出 | unwind/返回路径不完整导致残留引用。 |
| `VmExceptionRoot` | `exception_handlers`, `pending_exception` | 安装异常处理器或抛出异常时 | handler 出栈 + pending 清空后 | handler 深度与栈深不一致导致状态污染。 |
| `VmObjectTableRoot` | `objects` + 原型/全局对象 ID 缓存 | 执行初始化创建对象/原型后 | 执行重置时清空；后续由 GC sweep 细化 | 原型 ID 失配影响属性查找与 `globalThis` 回退。 |
| `ClosureRoot` | `closures`, `closure_objects` | 函数实例化并捕获外层环境时 | 函数对象不可达且无外部引用时 | 捕获链循环引用导致泄漏风险。 |
| `HostFunctionRoot` | `host_functions` | 注册 host function 或建立桥接句柄时 | 句柄注销或执行上下文销毁时 | host 侧仍引用但 VM 已释放，触发悬挂句柄问题。 |
| `Phase6ModuleRoot` (预留) | 模块缓存 | module instantiate 缓存建立时 | 缓存淘汰/realm 销毁时 | 模块循环依赖图中错误淘汰。 |
| `Phase6JobQueueRoot` (预留) | Promise 微任务队列 | job 入队时 | job 执行完成并无后续引用时 | 队列泄漏或错误提前回收。 |
| `HostPinRoot` | 宿主持有对象 pin 表（`pin_host_value`/`unpin_host_value`） | host 明确 pin 时 | host unpin 后 | pin/unpin 不配对导致泄漏或 use-after-free 风险。 |

## 3. Root 生命周期规则（v0）

1. 执行初始化规则：
`execute_in_realm` 开始时初始化 VM 执行态与对象缓存，形成一次执行的 root 边界。

2. 调用帧规则：
函数调用进入时新增栈/作用域根，返回或异常 unwind 时按深度回退并释放对应根。

3. 异常规则：
抛出异常后，`pending_exception` 必须在“被 catch 消费”或“向上抛出完成”后清理，避免错误保活。

4. 全局规则：
`Realm.globals` 默认为长期根；仅在 realm 生命周期结束时整体释放。

5. 宿主规则：
host function 与未来 host pin 必须有显式注册/反注册 API，禁止隐式长生命周期引用。

## 4. Mark-Sweep 接口契约（草案）

| 接口 | 输入 | 输出 | 约束 |
| --- | --- | --- | --- |
| `collect_roots()` | `Vm` + `Realm` 当前状态 | root 快照（按类别分组） | 快照必须稳定，不在遍历中修改容器。 |
| `mark_from_roots(roots)` | root 快照 | 可达对象集合 | 需覆盖对象属性、闭包捕获、原型链。 |
| `sweep_unreachable(marked)` | 可达对象集合 | 回收统计（回收数/剩余数） | 不得回收任何仍被 root 引用的对象。 |
| `gc_stats()` | 无 | 调试统计结构 | 用于回归测试与性能基线对比。 |

## 5. 与长期任务步骤的映射

- Step 1：对象/容器盘点 -> 已完成，见 `docs/memory-inventory.md`。
- Step 2：root 策略定义 -> 本文档即产出。
- Step 3：将本文“接口契约”细化为 `gc-design`。
- Step 4：为各 root 类别补齐回归用例。

## 6. Step 2 验收记录

- 每类 root 均已定义进入条件/退出条件/错误场景。
- 已预留 Phase 6 所需 root 类（模块缓存、微任务队列、host pin）。
- 可直接进入 Step 3（mark-sweep 设计与接口细化）。
