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
- `test262 language --max-cases 5000` 最新快照：`passed=4361`、`failed=639`（命令见 `docs/test262-baseline.md`）。
- 本轮新增语义收敛：
  - `obj.m()` / `obj[k]()` 调用已通过 `CallMethod*` 保留 receiver 绑定。
  - 标识符调用新增 reference-aware 路径（`CallIdentifier*`），修复 `with (obj) { method(); }` 的 `this` 绑定。
  - `super` 运行时回退链路在对象方法场景可用（`{ __proto__: proto, m() { return super.x; } }`）。
  - baseline 内建补齐 `parseInt`、`parseFloat`、`isFinite`。
  - 字符串词法补齐：前导小数字面量、`\u{...}` code point 转义、`\uD800-\uDFFF` surrogate 转义最小支持。
  - VM 关系运算中的字符串比较改为按 UTF-16 code unit 顺序（与 JS 规范/QuickJS 行为方向一致）。

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
4. class/constructor/super-call 与模板字符串仍是 language 子集主失败簇（当前失败集中在 `statements/class`、`expressions/template-literal`）。

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
