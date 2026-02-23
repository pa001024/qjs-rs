# qjs-rs AGENTS.md

## 0. 状态注记（2026-02-23）
- 当前仓库已不再停留在“仅 Phase 0 脚手架”：
  - workspace、CI、核心 crates 均已存在并可通过 `cargo test`。
  - `parser -> bytecode -> vm -> runtime -> builtins -> test-harness` 链路可执行。
- 阶段路线与验收标准仍按本文继续推进；当前真实进度请以 `docs/current-status.md` 为准。

## 1. 目标
- 构建一个**纯 Rust** 的 JavaScript 运行时库，语义上优先对齐 QuickJS。
- 参考实现：
  - QuickJS 源码：`D:\dev\QuickJS`
  - Boa（纯 Rust JS 引擎）：`D:\dev\boa`
- 当前仓库已具备可运行实现；后续迭代默认先完成方案/架构评审，再推进对应代码改动。

## 2. 约束与边界
- Runtime 核心禁止依赖 C FFI（工具链/脚本可用 Rust 生态工具）。
- 优先顺序：语义正确性 > 可维护性 > 性能。
- 先实现 ECMAScript 核心能力，再补充高级特性与优化。
- 默认以库形态交付（后续可加 CLI 壳）。

## 3. 总体迁移策略
- 不做“逐文件机械翻译”，做“语义对齐 + Rust 化重构”：
  - 从 QuickJS 提炼行为模型、边界条件、测试样例。
  - 借鉴 Boa 的模块划分、错误处理、Rust 风格 API。
  - 在 qjs-rs 中建立清晰分层：`parser -> ir/bytecode -> vm -> runtime -> builtins`。

## 4. 分阶段执行计划

### Phase 0: 基线与脚手架
- 建立 Cargo workspace（建议）：
  - `crates/lexer`
  - `crates/parser`
  - `crates/ast`
  - `crates/bytecode`
  - `crates/vm`
  - `crates/runtime`
  - `crates/builtins`
  - `crates/test-harness`
- 输出对齐文档（必须）：
  - `docs/quickjs-mapping.md`：QuickJS 模块到 Rust 模块映射
  - `docs/semantics-checklist.md`：语义检查清单
  - `docs/risk-register.md`：风险与应对

### Phase 1: 词法/语法前端
- 实现 Lexer + Parser + AST（先覆盖脚本执行主路径）。
- 支持严格模式、作用域声明、函数/闭包、对象/数组字面量、控制流。
- 建立语法快照测试 + parser 单测。

### Phase 2: 编译与字节码
- AST 降级到中间表示（IR）并编译为字节码。
- 先支持同步执行主路径，再补齐复杂控制流与异常跳转。
- 定义稳定 opcode 文档和反汇编调试输出。

### Phase 3: VM 与运行时核心
- 实现 `JSValue`、对象模型、原型链、属性描述符。
- 完成执行上下文、词法环境、闭包捕获、`this` 绑定。
- 覆盖 `eval`、函数调用、异常传播。

### Phase 4: GC 与内存模型
- 首版实现可验证正确性的 mark-sweep。
- 明确 Root 管理策略（栈、全局对象、模块缓存、任务队列）。
- 压测循环引用与高频分配场景。

### Phase 5: 内建对象与语言特性扩展
- 最小可用集合：
  - `Object`, `Function`, `Array`, `String`, `Number`, `Boolean`, `Math`, `Date`
  - `Error` 体系
  - `JSON`
- 第二批：
  - `Promise`, `Map/Set`, `RegExp`, `Symbol`, `BigInt`

### Phase 6: 模块系统与作业队列
- ES Module 解析、实例化、执行流程。
- Promise Job Queue（微任务）与宿主回调接口。

### Phase 7: 兼容性与性能收敛
- 接入 test262（先子集后扩容）。
- 对齐 QuickJS 样例与回归测试。
- 与 Boa/QuickJS 做基准对比，定位热点并优化。

## 5. 里程碑与验收标准
- M0（架构就绪）：workspace + 文档 + CI 基础通过。
- M1（可执行脚本）：可跑基础 JS 脚本，语法/执行单测通过。
- M2（语言核心闭环）：函数、对象、异常、闭包稳定。
- M3（内建可用）：主流内建对象可运行核心用例。
- M4（模块+Promise）：模块加载与微任务链路完整。
- M5（兼容性门槛）：test262 子集达到目标通过率（阈值后定）。
- M6（发布候选）：API 稳定、回归稳定、性能达标（阈值后定）。

## 6. 测试与质量策略
- 三层测试：
  - 单元测试（数据结构/算法/语义点）
  - 集成测试（脚本级行为）
  - 兼容测试（test262 + QuickJS 用例）
- 每个新特性必须附：
  - 至少 1 个正向用例
  - 至少 1 个边界/异常用例
- CI 最低门槛：
  - `cargo test`
  - `cargo clippy -- -D warnings`
  - `cargo fmt --check`

## 7. 风险与应对
- 风险：QuickJS 内部优化与 C 细节难以直接映射 Rust。
  - 应对：优先语义一致，内部实现允许不同。
- 风险：GC 与借用规则冲突导致开发阻塞。
  - 应对：先用可维护的句柄/索引模型，再做性能优化。
- 风险：一次性覆盖全部特性导致周期失控。
  - 应对：按里程碑分批交付，先核心再扩展。

## 8. 下一步（等待确认后执行）
- 按 Phase 0 创建 workspace 与文档骨架。
- 先完成 `JSValue` 表示方案评审（`enum` vs NaN-boxing）并确定首版。
- 落地最小 parser + REPL 执行链路（可运行 `1 + 2` 与函数调用）。
