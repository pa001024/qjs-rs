# Current Status Snapshot

基线日期：2026-02-23

## 1. 复核范围

- 代码结构：workspace、crate 布局、CI 配置。
- 运行质量：`cargo test -q`。
- 规划对齐：Phase 0~7 当前状态与下一阶段缺口。

## 2. 关键结果

- 工作区结构完整：`crates/ast`、`crates/lexer`、`crates/parser`、`crates/bytecode`、`crates/vm`、`crates/runtime`、`crates/builtins`、`crates/test-harness`。
- CI 已存在并覆盖格式化/静态检查/测试：`.github/workflows/ci.yml`。
- CI 已接入 GC guard stress gate（`test262-run --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline`），用于持续监控 runtime/reclaimed 统计回归。
- 本地复核 `cargo test -q` 全部通过（0 失败）。
- `test262 language --max-cases 5000` 最新快照：`passed=4560`、`failed=440`（命令见 `docs/test262-baseline.md`）。
- 本轮新增语义收敛：
  - `obj.m()` / `obj[k]()` 调用已通过 `CallMethod*` 保留 receiver 绑定。
  - 标识符调用新增 reference-aware 路径（`CallIdentifier*`），修复 `with (obj) { method(); }` 的 `this` 绑定。
  - `super` 运行时回退链路在对象方法场景可用（`{ __proto__: proto, m() { return super.x; } }`）。
  - baseline 内建补齐 `parseInt`、`parseFloat`、`isFinite`。
  - 字符串词法补齐：前导小数字面量、`\u{...}` code point 转义、`\uD800-\uDFFF` surrogate 转义最小支持。
  - VM 关系运算中的字符串比较改为按 UTF-16 code unit 顺序（与 JS 规范/QuickJS 行为方向一致）。
  - 数值词法补齐：十六进制字面量 `0x.../0X...`。
  - parser 在 `Expression` 上下文（statement / if-while-do / for 条件更新 / return / throw / switch / with）补齐逗号运算符序列解析，避免把合法 `a, b` 误判为语句分隔错误。
  - lexer 对齐 QuickJS 风格新增 `is_regexp_allowed` 路径：在允许正则的上下文将 `/.../flags` 作为单 token 扫描，修复 regexp 字面量中的 `\` 词法失败。
  - lexer/parser 新增 template literal 分段 token 与解析（含 cooked/raw 区分、line-continuation raw 保真、tagged template 最小调用降级）。
  - tagged template 首参已从“仅 cooked 数组”升级为“cooked 数组 + `raw` 数组属性”，并补齐 `new tag\`...\`` 优先级（tagged template 高于 `new` 构造解析）。
  - template invalid escape 场景在 tagged template 下不再 parse-fail，改为 cooked `undefined` + raw 保留，收敛 `tagged-template/invalid-escape-sequences.js`。
  - class lowering 改为始终生成函数构造器（含空 class），并将实例方法改为 `Object.defineProperty`（`enumerable: false`）定义。
  - VM 函数对象补齐“显式 `[[Prototype]]` 改写”状态：`Object.setPrototypeOf(fn, null)` 后不再错误回退到 `Function.prototype`。
  - `Object.defineProperty` 已支持函数闭包目标，函数属性访问/写入补齐 accessor 路径，修复 class static computed `constructor` getter/setter 失败簇。
  - 构造路径移除“实例强制写入自有 `constructor`”行为，恢复通过原型链解析 `constructor`，修复 `class { ['constructor']() {} }` 语义偏差。
  - bytecode 将 `var` 初始化改为 reference-aware PutValue 路径（`ResolveIdentifierReference + StoreReferenceValue`），修复 `with` 语句内 `var x = ...` 错误绕过对象环境的问题。
  - regex 运行时最小可用链路增强：regex literal 改为调用 `RegExp(pattern, flags)`、`RegExp` 对象补齐 `global/ignoreCase/multiline/unicode/sticky/dotAll/lastIndex` 属性，并新增 `test()` host 路径（Rust regex backend），收敛 `literals/regexp` 的 `NotCallable` 与 `instanceof` 失败。
  - class method/accessor 函数新增“不可构造、无 prototype”标记，VM 在 `new`/`in`/属性读取路径按该标记处理，进一步对齐 class 方法行为。
  - 函数 `length` 从“形参总数”修正为“首个默认参数前的形参数量”（含 class/object/arrow/function 默认参数场景），清理 `dflt-params-trailing-comma` 失败簇。
  - parser 新增可选 `catch` 绑定语法（`catch { ... }`），修复 `scope-catch-param-*` parse 失败簇。
  - bytecode 修复 `switch` 与 `try/catch` completion value 传播（保留分支最后求值结果，不再统一丢成 `undefined`），清理一批 `statements/(switch|try)/cptn-*` 失败。
  - class lowering 对齐 descriptor 细节：`C.prototype` 改为不可写/不可配/不可枚举，static method 统一经 `Object.defineProperty(enumerable:false)` 定义；同时 VM 跳过内部 class 临时名推断，修复 `class/definition` 中 `basics/methods/prototype-property` 失败。
  - bytecode 的 statement-list 最后取值目标改为跳过 `var/let/const/function/empty` 空完成值语句，并修复 `var` 初始化的栈残留（`StoreReferenceValue` 后补 `Pop`），进一步清理 `statements/{class,const,empty,let,variable}/cptn-*` 失败簇。
  - runtime/builtins 将 `Error/TypeError/ReferenceError/SyntaxError/EvalError/RangeError/URIError` 拆分为独立 Native constructor，避免全部错误落成 `Test262Error` 字符串前缀。
  - VM `instanceof` 收敛：错误构造器匹配从“泛 Error”改为按构造器名精确匹配；同时补齐 RHS `prototype` 非对象时的 TypeError 与对象左值原型链匹配。
  - VM `in`/`instanceof` 运行时异常已统一接入 handler 路由，可被 `try/catch` 捕获（不再直接顶层失败）。
  - String baseline 补齐 `String.prototype.split(separator, limit)` 最小可运行路径，并在字符串属性可见性里暴露 `split`。
  - `DefineVariable` 重声明写回策略收敛：`undefined` 仅对内部临时名（`$__loop_completion_`/`$__switch_tmp_`/`$__class_ctor_`）回写，避免污染用户 `var/function` 绑定。
  - 标识符引用回退路径补齐：`globalThis`/`Math`/`this`/realm globals/global object 属性可在 `Unresolvable` 路径读取，降低 `UnknownIdentifier` 噪声。

## 3. 分阶段状态

| Phase | 状态 | 证据 | 当前结论 |
| --- | --- | --- | --- |
| Phase 0 | Done | `Cargo.toml`, `docs/quickjs-mapping.md`, `docs/semantics-checklist.md`, `docs/risk-register.md`, `.github/workflows/ci.yml` | 脚手架与基础治理已具备。 |
| Phase 1 | In Progress | `crates/lexer/src/lib.rs`, `crates/parser/src/lib.rs`, `crates/ast/src/lib.rs` | 前端主路径可运行，继续补齐语义边角。 |
| Phase 2 | In Progress | `crates/bytecode/src/lib.rs` | 指令与编译链路已建立，控制流/异常语义持续收敛。 |
| Phase 3 | In Progress | `crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs` | 执行链路可用，仍需进一步完善对象模型与边界语义。 |
| Phase 4 | In Progress | `docs/memory-inventory.md`, `docs/root-strategy.md`, `docs/gc-design.md`, `docs/gc-test-plan.md`, `crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`, `docs/phase4-review.md` | GC 方案、测试计划、PoC、评审与 `ObjectId(slot+generation)` 句柄加固已完成，进入下一轮压力验证与策略细化。 |
| Phase 5 | In Progress | `crates/builtins/src/lib.rs` | 已有 baseline 内建，需继续扩展规范行为。 |
| Phase 6 | Planned | `crates/parser/src/lib.rs`, `crates/vm/src/lib.rs` | ES Module 与微任务队列尚未接通。 |
| Phase 7 | In Progress | `docs/test262-lite.md`, `docs/test262-baseline.md`, `crates/test-harness` | 已有兼容性回归链路，但通过率仍需系统提升。 |

## 4. 当前主要缺口

1. GC 已落地首版 mark-sweep，但仍缺增量/分代策略与更大规模性能压测。
2. `eval/with/strict` 与 descriptor 等复杂语义仍需持续压测与修正。
3. 模块系统与 Promise job queue 尚未启动实现。
4. 函数/eval 与 class 继承链语义仍是 language 子集主失败簇（当前失败集中在 `eval-code/*`、`statements/class`、`statements/function`），其中 `eval-code/direct` 的 arguments/var 环境交互仍需重点收敛。

## 5. 下一步执行

- 执行长期任务：`docs/long-horizon-task-phase4.md`（总时长 >8h，含子 agent 并行方案）。
- Phase 4 已完成前六步推进：
  - Step 1: `docs/memory-inventory.md`
  - Step 2: `docs/root-strategy.md`
  - Step 3: `docs/gc-design.md`
  - Step 4: `docs/gc-test-plan.md`
  - Step 5: `crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`（最小 GC PoC）
  - Step 6: `docs/phase4-review.md`（集成评审与风险收口）
- Phase 4 Step 7 已完成：`crates/vm/src/lib.rs`（`ObjectId slot+generation` 句柄加固 + stale handle 回归）。
- Phase 4 Step 8 已完成（首轮）：新增 8 个 GC 压力样例并完成 19/19 回归，为 Step 10 规模化扩展建立基线。
- Phase 4 Step 9 完成：Default/Stress profile 触发/观测校验闭环，命令与快照都符合预期。
  - Default Profile command: `test262-run --show-gc`（默认 auto/runtime 关闭）with VM regression asserting `gc_stats == GcStats::default()` (zeroed counters) and `boundary_collections == collections_total` while `runtime_collections == 0`; latest snapshot `collections_total=0`, `boundary_collections=0`, `runtime_collections=0`.
  - Stress Profile command: `test262-lite --auto-gc --runtime-gc --auto-gc-threshold 1 --runtime-gc-interval 1` plus `test262-run --show-gc` snapshot showing `collections_total=29283`, `boundary_collections=22`, `runtime_collections=29261`, `reclaimed_objects=611` and confirming `collections_total == runtime_collections + boundary_collections`.
- Phase 4 Step 10 已启动：GC 压测样例已扩展至 26 个总样例（含 18 个 `gc-*`），并新增快照报告 `docs/gc-snapshot-report.md`。
- 自动 GC 已支持开关+阈值（执行边界触发，默认关闭）。
- `test262-lite` 已接入 `gc-*` 样例（闭包捕获、异常 unwind、with、闭包链、循环引用、循环闭包）并在集成测试中启用自动 GC 压测模式。
- `test262-run` CLI 已支持 `--auto-gc` / `--auto-gc-threshold`。
- `test262-run` CLI 已支持 `--runtime-gc` / `--runtime-gc-interval`（安全点模式）。
- `test262-run` CLI 已支持 GC guard 阈值参数与基线文件模式：`--expect-gc-baseline` + `--expect-*`（显式参数优先），可作为 CI 回归门槛。
- VM 已支持运行中安全点 GC（`enable_runtime_gc` + `set_runtime_gc_check_interval`）。
- `gc_stats` 已提供对象规模与 mark/sweep 耗时观测字段。
- 已接入 `HostPinRoot` 最小 API（pin/unpin）并有回归测试覆盖。
- VM 已接入 `ObjectId(slot+generation)`，并新增 stale handle 回归测试确保回收复用安全。
- `test262-lite` 在 `--auto-gc --runtime-gc` 模式下当前 26/26 通过。
- `test262-run --show-gc` 已可输出套件级 GC 聚合统计；最新 stress 快照：`collections_total=29283`、`boundary_collections=22`、`runtime_collections=29261`、`reclaimed_objects=611`。
- `crates/test-harness/tests/test262_lite.rs` 已增加 GC 守护断言：`reclaimed_objects > 0` 且 runtime ratio `>= 0.9`。
- `array churn + runtime GC` 的 `UnknownObject` 问题已通过 `gc_shadow_roots` 修复并加入 VM 回归测试，进入持续监控阶段。
- 以 test262 失败簇驱动 builtins 与语义缺口收敛，持续更新基线文档。
