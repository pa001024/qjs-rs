# Long Horizon Task: Phase 4 GC & Memory Model

目标日期：2026-02-23 起  
总预计时长：18.0 小时（>8 小时）  
目标：在不引入 C FFI 的前提下，为 `qjs-rs` 建立首版可验证的 GC/Root 方案，并把落地风险前置收敛。

## 执行进度

- [x] Step 1 已完成文档化：`docs/memory-inventory.md`
- [x] Step 2 已完成文档化：`docs/root-strategy.md`
- [x] Step 3 已完成文档化：`docs/gc-design.md`
- [x] Step 4 已完成文档化：`docs/gc-test-plan.md`
- [x] Step 5 已完成 PoC：`crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`
- [x] Step 6 已完成评审：`docs/phase4-review.md`
- [x] Step 7 已完成句柄加固：`crates/vm/src/lib.rs`（`ObjectId slot+generation` + 回归测试）
- [x] Step 8 已完成：GC 压力与失败簇扩展（首轮新增 8 个 `gc-*` fixture 并通过；`test262-lite --auto-gc --runtime-gc` 首轮基线 19/19）
- [x] Step 9 完成：Default/Stress profile 触发与观测校验闭环，CLI `test262-run --show-gc` 产出最新快照且所有回归通过。
  - Default Profile command: `test262-run --show-gc`（默认 auto/runtime 关闭）with regression assertions `gc_stats == GcStats::default()` and `boundary_collections == collections_total` while `runtime_collections == 0`.
  - Default Profile snapshot: `collections_total=0`, `boundary_collections=0`, `runtime_collections=0`.
  - Stress Profile command: `test262-lite --auto-gc --runtime-gc --auto-gc-threshold 1 --runtime-gc-interval 1` plus `test262-run --show-gc` snapshot showing `collections_total=29283`, `boundary_collections=22`, `runtime_collections=29261`, `reclaimed_objects=611` and confirming `collections_total == runtime_collections + boundary_collections`.
- [ ] Step 10 进行中：GC 压测规模化与性能门槛（已扩展到 26/26，总计 18 个 `gc-*` fixture；持续监控+阈值治理）

## 任务分步骤拆解

### Step 1 (1.5h): 对象与分配路径盘点
- 目标：明确当前对象创建、持有、释放缺口的真实路径。
- 输入：`crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`, `crates/builtins/src/lib.rs`。
- 执行：梳理对象存储结构、引用路径、潜在泄漏路径，标注“必须进 root set”的来源。
- 产出：`memory-inventory` 表（对象类型 -> 所在容器 -> root 来源 -> 回收风险）。
- 验收：至少覆盖 VM 栈、全局对象、闭包环境、异常栈、内建缓存五类来源。

### Step 2 (2.0h): Root 集合策略设计
- 目标：定义首版 root 集合边界与生命周期规则。
- 输入：Step 1 盘点结果、`docs/risk-register.md`。
- 执行：设计 root 分类（执行栈、全局、模块缓存、任务队列预留、host pin），给出注册/反注册时机。
- 产出：`root-strategy` 设计稿（数据结构 + 生命周期图）。
- 验收：每类 root 都有“进入条件/退出条件/错误处理”定义。

### Step 3 (2.5h): Mark-Sweep 方案与接口契约
- 目标：确定首版 mark-sweep 算法细节与 VM/runtime 接口边界。
- 输入：Step 2 root 策略、现有 `JsValue`/对象容器实现。
- 执行：定义 mark 阶段遍历入口、sweep 阶段删除策略、暂停点与调试钩子。
- 产出：`gc-design.md` 草案 + 接口列表（`mark_from_roots`, `sweep`, `gc_stats` 等）。
- 验收：覆盖循环引用案例；接口可被 test-harness 驱动验证。

### Step 4 (2.0h): 语义与回归验证方案
- 目标：确保 GC 引入后不破坏现有语义与执行链路。
- 输入：`docs/semantics-checklist.md`, `crates/test-harness`。
- 执行：设计 GC 相关回归矩阵（闭包捕获、异常路径、prototype 链、数组/对象混合分配）。
- 产出：`gc-test-plan`（测试清单 + 验收标准 + 失败分级）。
- 验收：每个高风险语义点至少 1 个正向 + 1 个边界用例。

### Step 5 (2.5h): 最小实现冲刺（PoC）
- 目标：实现可运行的首版 GC PoC（先正确性后性能）。
- 输入：Step 3 接口契约、Step 4 测试计划。
- 执行：按最小闭环实现 mark-sweep，打通触发入口并输出调试统计。
- 产出：PoC 代码 + 回归结果 + 已知限制清单。
- 验收：`cargo test -q` 维持全绿，新增 GC 专项用例通过。

### Step 6 (2.0h): 集成评审与风险收口
- 目标：完成阶段评审并形成下一轮 backlog。
- 输入：Step 1~5 全部产出。
- 执行：对照 `docs/risk-register.md` 更新风险状态，产出后续优化清单（暂停时间、吞吐、可观测性）。
- 产出：`phase4-review` 报告 + 下一轮任务分解。
- 验收：R-002 状态从 Open 进入 In Progress，并有可执行后续项。

### Step 7 (1.0h): ObjectId 代际句柄加固
- 目标：消除对象回收后 slot 复用造成 stale handle 误命中风险。
- 输入：Step 5 PoC、`docs/gc-design.md`、`docs/gc-test-plan.md`。
- 执行：将 `ObjectId` 编码为 `slot+generation`，回收只复用 slot，重分配时提升 generation。
- 产出：VM 代际句柄实现 + stale handle 回归测试 + 文档同步。
- 验收：回收后复用 slot 时旧句柄访问返回 `UnknownObject`，新句柄读写正常。

### Step 8 (2.5h): GC 压力与失败簇扩展
- 目标：在高频分配/回收场景下验证正确性稳定，不引入语义回退。
- 输入：`crates/test-harness`、现有 `gc-*` fixtures、`docs/test262-lite.md`。
- 执行：扩展 `gc-*` 场景并增加长链对象、循环引用、闭包深层捕获压力样例。
- 产出：新增压力样例、失败簇清单、回归结果快照。
- 验收：`cargo test -q` 与 `test262-lite --auto-gc --runtime-gc` 维持全绿。

### Step 9 (2.0h): 观测与触发策略细化
- 目标：让运行中 GC 触发策略更可控且可观测，降低语义回归风险。
- 输入：`crates/vm/src/lib.rs`、`docs/gc-design.md`、`docs/risk-register.md`。
- 执行：补充触发统计口径与阈值策略说明，形成“默认策略 + 压测策略”双配置。
- 产出：策略文档更新、必要的回归断言、风险状态更新。
- 验收：R-002 由 `In Progress` 进入 `Low/Monitoring` 的进入条件明确可执行。
  - Default Profile：`auto_gc=false` 且 `runtime_gc=false` 时 `gc_stats == GcStats::default()`；开启 `auto_gc` 且关闭 `runtime_gc` 时满足 `boundary_collections == collections_total`、`runtime_collections == 0`。
  - Stress Profile：`test262-lite --auto-gc --runtime-gc`（auto threshold=1、runtime interval=1）持续通过，且 VM 统计满足 `runtime_collections > 0`、`collections_total == runtime_collections + boundary_collections`。
  - 观测产出需附带两个 profile 下的 `gc_stats` 快照与实际 command 输出，便于后续风险复审。

### Step 10 (2.5h): GC 压测规模化与性能门槛
- 目标：在保持语义正确性的前提下，建立可持续监控的 GC 压测门槛。
- 输入：`crates/test-harness`、`docs/gc-test-plan.md`、`docs/risk-register.md`。
- 执行：
  - 扩展 `gc-*` 样例到 24+ 并按风险簇分类（closure/cycle/exception/array-churn）。
  - 固化 `--show-gc` 快照采样流程（default + stress），形成周度对比基线。
  - 建立异常阈值告警口径（例如 `collections_total` 与 `runtime_collections` 比例突变）。
- 产出：规模化压力样例、快照对比表、GC guard 阈值门槛化清单（命令+阈值/基线文件）、Step 10 验收记录。
- 验收：压力样例稳定全绿且 `--show-gc` 快照可复现，R-002 维持 Monitoring 且无回归升级。

## 子 agent 并行提效方案

并行目标：缩短“设计 -> 验证 -> 实现”串行等待时间，减少单线程上下文切换成本。

1. Agent A（Explorer）: 内存路径盘点
- 负责 Step 1 + Step 2 的事实收集与 root 候选清单。
- 交付：`memory-inventory` 初稿、root 分类建议。

2. Agent B（Explorer）: 算法与接口草案
- 负责 Step 3，输出 mark-sweep 设计、接口签名、边界条件列表。
- 交付：`gc-design.md` 草案结构与关键伪代码。

3. Agent C（Explorer/Worker）: 验证与回归计划
- 负责 Step 4，梳理需要新增的测试场景与验收口径。
- 交付：`gc-test-plan` 与最小测试集清单。

4. 主 agent（当前）: 合并与落地
- 合并 A/B/C 输出，执行 Step 5 实现冲刺与 Step 6 收口评审。
- 交付：可运行 PoC、更新后的风险文档、下一轮 backlog。

## 下一轮并行执行包（>=8h）

### Workstream A（3.0h）: GC fixture 扩展
- Owner: Explorer A + Worker A
- 输入：`crates/test-harness/fixtures/test262-lite/pass/gc-*.js`
- 目标：新增循环引用、深层原型链、嵌套闭包压力场景。
- 验收：`test262-run --auto-gc --runtime-gc` 全绿，失败簇归零或有明确归因。

### Workstream B（2.5h）: 触发策略与观测增强
- Owner: Explorer B + Worker B
- 输入：`crates/vm/src/lib.rs`, `docs/gc-design.md`
- 目标：补充 GC 触发统计与阈值策略（默认模式/压测模式）。
- 验收：`gc_stats` 指标可用于区分执行边界触发与运行中触发。

### Workstream C（1.5h）: 风险与验收口径收敛
- Owner: Explorer C
- 输入：`docs/risk-register.md`, `docs/gc-test-plan.md`, `docs/current-status.md`
- 目标：更新 R-002 收敛条件，形成可量化“Low/Monitoring”准入标准。
- 验收：文档口径一致，下一轮 gate 可直接执行。

### 集成与回归（1.5h）
- Owner: 主 agent
- 执行：合并 A/B/C 产物，跑 `cargo test -q` 与 test262-lite GC 压测。
- 输出：阶段快照 + 后续迭代入口条件。

## 完成定义（Definition of Done）

- 有可执行 GC PoC，且不破坏现有测试基线。
- GC root 策略和接口契约形成文档化结论，可被后续迭代复用。
- 风险登记与语义清单同步更新，文档状态与代码状态一致。
