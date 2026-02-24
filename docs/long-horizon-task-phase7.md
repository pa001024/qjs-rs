# Long Horizon Task Plan (Phase 7 Compatibility Push)

基线日期：2026-02-24
目标：以 QuickJS 语义对齐为前提，持续推进 test262 兼容性（当前样本：`5000/0`、`10000->5265/0`）。
预计总时长：`>= 10h`（建议按 5 个 2h 冲刺执行）。

## 1. 迁移原则（强约束）

- 先对照 `D:\dev\QuickJS\quickjs.c` 的编译/执行路径，再在 qjs-rs 落地。
- 禁止“自创语义捷径”绕过 QuickJS 与 ECMAScript 已知行为。
- 每个冲刺必须：`cargo fmt`、核心 crate 测试、至少一个 test262 子集回归、一次 git 提交。

## 2. 下一轮扩容模块拆分（并行）

### Track A: language 样本扩容与回归守护（2h）

- 模块范围：
  - `language` 更大样本扩容（逐步放宽 `--max-cases`）
  - `language` 全量抽样 smoke
  - eval/regexp/with 历史高风险簇回归守护
- QuickJS 对照点：继续以 `quickjs.c` 的 regexp/parser 主路径做行为锚点，防止回归。
- 目标产物：
  - 将 `5000/0`、`10000->5265/0` 结果扩展到更大样本并保持单调不回退
  - 输出可持续 nightly 基线快照与守护阈值

### Track B: let 循环作用域与 TDZ（2h）

- 模块范围：
  - `statements/let/*closure*`
  - per-iteration fresh binding
- QuickJS 对照点：for-loop per-iteration environment 创建与闭包捕获。
- 目标产物：
  - `for (let ...)` 每轮独立绑定
  - 初始化前赋值/读取正确抛错

### Track C: Tagged Template 缓存与冻结（2h）

- 模块范围：
  - `expressions/tagged-template/cache-*`
  - `template-object-frozen-*`
- QuickJS 对照点：template object cache（按 site 缓存）与 raw/cooked 冻结语义。
- 目标产物：
  - 同一源码位置对象复用
  - `template` 与 `template.raw` 冻结、不可扩展

### Track D: Destructuring Assignment（2h）

- 模块范围：
  - `expressions/assignment/destructuring/*`
- QuickJS 对照点：assignment target grammar 与 iterator close/abrupt completion。
- 目标产物：
  - 解析通过（消除 parse-fail）
  - iterator return/get throw 链路行为对齐

### Track E: Async Function 最小闭环（2h）

- 模块范围：
  - `expressions/async-function/*`
  - `statements/async-function/*`
  - `statements/for/head-init-async-of.js`
- QuickJS 对照点：async function 返回 Promise 实例、body 求值与错误传播。
- 目标产物：
  - async declaration/expression 返回 Promise
  - parser 支持当前失败样例语法形状

## 3. 并行执行模板（子 agent 优先）

说明：本仓库并行默认优先子 agent；若遇到线程上限，回退为 shell 并行命令。

- Agent A（explorer）：只做 QuickJS 对照定位，输出函数名/分支与行为结论。
- Agent B（worker）：实现 parser/bytecode 侧修复（只改对应模块文件）。
- Agent C（worker）：实现 vm/runtime/builtins 侧修复（只改对应模块文件）。
- 主 agent：合并、回归、文档、提交。

每个 track 的提交格式：
- Commit 标题：`phase7/<track>: <short semantic fix summary>`
- 必含内容：代码 + 回归测试 + `docs/test262-baseline.md` / `docs/current-status.md` 更新。

## 4. 每轮冲刺固定步骤

1. 对照 QuickJS 对应分支并记录结论。
2. 在 qjs-rs 定位同类路径并最小修复。
3. 运行：
   - `cargo test -q -p parser`
   - `cargo test -q -p bytecode`
   - `cargo test -q -p vm --lib`
   - 对应 test262 子集（`--show-failures`）
4. 更新文档并提交。

## 5. 验收门槛

- 过程门槛：每 2h 至少 1 次功能提交。
- 阶段门槛：5000 与 10000 样本基线保持清零，并持续推进更大样本通过率。
- 终局门槛：失败簇集中后切换到全量 test262 计划，并建立 nightly 基线快照。

## 6. 最新进展与下一轮（>=8h）

- 已完成：
  - `language --max-cases 5000`: `5000/0`
  - `language --max-cases 10000`: `5265/0`
  - `built-ins/Object`: `2255/0`（`target/test262-builtins-object-20260224-v96.json`）
  - `built-ins/Array --max-cases 100`: `100/0`（`target/test262-builtins-array-20260224-v4-s100.json`）
  - `built-ins/Array/length`: `26/0`（`target/test262-builtins-array-length-20260225-v5-full.json`）
  - `built-ins/Array/of`: `9/0`（`target/test262-builtins-array-of-20260225-v1.json`）
  - `built-ins/Array/prototype/concat`: `14/0`（`target/test262-array-prototype-concat-20260225-v2.json`）
  - `built-ins/Array/prototype/copyWithin`: `12/0`（`target/test262-array-prototype-copyWithin-20260225-v3.json`）
  - `built-ins/Array/prototype/every`: `210/0`（`target/test262-array-prototype-every-20260225-v3.json`）
  - `built-ins/Array/prototype/fill`: `8/0`（`target/test262-array-prototype-fill-20260225-v1.json`）
  - `built-ins/Array/prototype/filter`: `220/0`（`target/test262-array-prototype-filter-20260225-v1.json`）
  - `built-ins/Array/prototype/find`: `11/0`（`target/test262-array-prototype-find-20260225-v1.json`）
  - `built-ins/Array/prototype/findIndex`: `11/0`（`target/test262-array-prototype-findIndex-20260225-v1.json`）
  - `built-ins/Array/prototype/forEach`: `182/0`（`target/test262-array-prototype-forEach-20260225-v1.json`）
  - `built-ins/Array/prototype/indexOf`: `192/0`（`target/test262-array-prototype-indexOf-20260225-v1.json`）
  - `built-ins/Array/prototype/join`: `16/0`（`target/test262-array-prototype-join-20260225-v2.json`）
  - `Array` 扩容采样（`--max-cases 300`）：`300/0`（`target/test262-builtins-array-20260225-v8-s300.json`）
  - `Array` 扩容采样（`--max-cases 1000`）：`1000/0`（`target/test262-builtins-array-20260225-v12-s1000.json`）
  - `Array.length` 超时根因已清理：按 QuickJS `set_array_length` 方向改为“仅删除已存在索引属性”，避免稀疏大索引 O(range) 退化。
- 下一轮并行模块拆分（建议 4 条线并行，每线 2~3h）：
  - Track F（Proxy 正式化）：补齐 `get/set/has/deleteProperty/getOwnPropertyDescriptor/defineProperty/ownKeys` trap 与不变量校验，对照 QuickJS `JSProxy` 路径。
  - Track G（TypedArray 扩展）：从当前 alias 过渡到真实 typed-array 家族构造器与 element 读写语义，覆盖 `Int8/Uint8Clamped/Int16/Uint16/Int32/Uint32/Float32/Float64/BigInt64/BigUint64`。
  - Track H（WeakMap/WeakSet 语义）：从 Map/Set alias 过渡到最小真实语义（对象键约束、`set/get/has/delete`），并补齐与 GC root 的交互约束。
  - Track I（全量基线推进）：开启 `test262` 更大样本/全量抽样与 nightly 快照，按失败簇持续回归清理。
- 每个 Track 必须输出：
  - QuickJS 对照点（函数名/分支）
  - 代码提交（最小可验证增量）
  - 对应 test262 子集快照与文档更新
