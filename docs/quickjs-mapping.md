# QuickJS to qjs-rs Mapping

基线日期：2026-02-23  
状态口径：`Done` / `In Progress` / `Planned`

| QuickJS area | qjs-rs crate | 状态 | 已落地能力（当前基线） | 下一步缺口 |
| --- | --- | --- | --- | --- |
| Tokenization | `crates/lexer` | In Progress | 覆盖常见运算符、关键标点、注释、字符串、Unicode 标识符与部分转义路径。 | 继续补齐边界词法与和 parser 联动的语义错误上下文。 |
| Parser / AST | `crates/parser`, `crates/ast` | In Progress | 支持脚本主路径、函数、控制流、异常、对象/数组字面量、class/arrow 等基线形状；class method/accessor 已注入“不可构造”标记。 | 模块语义、早期错误覆盖深度与复杂语法边角仍需收敛。 |
| Bytecode compiler | `crates/bytecode` | In Progress | 已形成 AST 到指令集的主降级路径，包含函数/作用域/跳转/异常处理相关 opcode。 | 继续稳定 completion record 与复杂控制流的编译约束。 |
| VM execution | `crates/vm` | In Progress | 可执行脚本链路，支持作用域栈、调用、异常传播、对象读写、部分内建交互；已新增 `CallMethod*` 与 `CallIdentifier*` 路径以对齐 receiver/base-object `this` 绑定语义；函数对象已支持 `Object.defineProperty` accessor 路径。 | `this`、严格模式细节、descriptor 细节与 corner cases 仍需对齐。 |
| Value/object model | `crates/runtime` | In Progress | 提供 `JsValue`、`Realm` 与对象存储基线，支撑当前执行路径；已提供 `Realm.globals_values()` 供 GC root 收集。函数闭包对象已区分默认原型与显式 `[[Prototype]]` 覆盖（含 `setPrototypeOf(fn, null)`）。 | 对象属性描述符细节、宿主对象生命周期与跨 realm 行为仍需收敛。 |
| Builtins | `crates/builtins` | In Progress | 已注入一批 baseline 全局对象与函数（含 `parseInt`/`parseFloat`/`isFinite` 与 test harness 运行所需最小集），并补齐 `EvalError/RangeError/URIError` 入口。 | 仍需系统化补齐 `JSON`、`Error` 族细节、`Promise` 等高阶内建。 |
| Compatibility harness | `crates/test-harness` | In Progress | 已有 `test262-lite` 跑批与 CLI，支持 frontmatter 驱动与失败样本导出。 | 真实 test262 覆盖、host hooks、严格模式 include 机制仍待扩展。 |
| GC / Memory model | `crates/vm`, `crates/runtime` | In Progress | 已完成 root 盘点、策略、GC 设计、测试计划与最小 mark-sweep PoC；已实现 `ObjectId(slot+generation)` 句柄防护。 | 继续强化运行中触发策略、压力回归覆盖与性能观测。 |
| Module / Job queue | `crates/parser`, `crates/runtime`, `crates/vm` | Planned | 尚无完整 ES Module 与微任务执行链路。 | Phase 6 需补齐解析、实例化、执行与 Promise job queue。 |

## Phase 对齐快照（2026-02-23）

| Phase | 状态 | 说明 |
| --- | --- | --- |
| Phase 0 | Done | workspace、基础文档、CI 已具备。 |
| Phase 1 | In Progress | lexer/parser/ast 主路径可运行，仍在补齐语义边界。 |
| Phase 2 | In Progress | bytecode 指令与编译主链路已建立，持续修正控制流与异常语义。 |
| Phase 3 | In Progress | VM/runtime/builtins 可跑大量用例，核心对象模型仍未闭环。 |
| Phase 4 | In Progress | GC/root 文档、PoC 与评审已完成，下一轮进入集成强化。 |
| Phase 5 | In Progress | 部分内建可用，仍需扩面与规范细节。 |
| Phase 6 | Planned | 模块系统与微任务队列尚未落地。 |
| Phase 7 | In Progress | 已有 test262 基线与回归机制，但通过率和覆盖仍需持续推进。 |

## 当前推进重点
1. 以 Phase 4 为主线推进 GC 从“正确性闭环”到“压力验证+策略细化”（详见 `docs/long-horizon-task-phase4.md`）。
2. 在不牺牲语义正确性的前提下，继续压降 `test262` 失败簇。
3. 统一文档状态口径，避免“阶段计划”和“真实实现”脱节。

## QuickJS 对照锚点（本轮）
- `D:\dev\QuickJS\quickjs.c:7276` `JS_SetPrototypeInternal`：对象原型可显式设为 `null`，用于对齐 `Object.setPrototypeOf(fn, null)` 行为。
- `D:\dev\QuickJS\quickjs.c:16519-16529` class heritage 校验：`extends` 目标需是 constructor，且其 `prototype` 必须为 object/null（支持 getter 取值路径）。
- `D:\dev\QuickJS\quickjs.c:16566` class 构造注释：`constructor` 属性先定义，但可被 computed property names 覆盖。
- `D:\dev\QuickJS\quickjs.c:38645` `js_object_defineProperty`：对所有 `JS_TAG_OBJECT` 生效（函数同属对象），用于对齐“函数目标上的 defineProperty/accessor”语义。
