# Phase 4 Review (GC & Memory Model)

评审日期：2026-02-23

## 1. 本轮目标与结论

- 目标：完成 Phase 4 的“设计 -> 验证 -> PoC -> 收口”最小闭环。
- 结论：闭环已达成，进入下一迭代（强化触发策略、扩展回归、增强宿主 pin 治理）。

## 2. 已交付产物

1. Step 1：`docs/memory-inventory.md`
2. Step 2：`docs/root-strategy.md`
3. Step 3：`docs/gc-design.md`
4. Step 4：`docs/gc-test-plan.md`
5. Step 5（PoC 代码）：
- `crates/vm/src/lib.rs`
- `crates/runtime/src/lib.rs`
6. Step 6（本评审）：`docs/phase4-review.md`

## 3. PoC 能力范围

- 已实现最小 `collect_roots -> mark_from_roots -> sweep_unreachable -> gc_stats` 路径（显式调用）。
- 已实现受控自动触发（执行边界、开关+阈值配置，默认关闭）。
- 已实现 `ObjectId(slot+generation)` 句柄防护，slot 复用时 generation 递增，避免 stale handle 误命中。
- 已补充 VM 回归测试：
  - 不可达对象可回收。
  - `Realm.globals` 可达对象不被回收。
  - 回收后复用 slot 时旧句柄访问返回 `UnknownObject`，新句柄读写正常。
  - 自动触发默认关闭/开启生效/阈值生效。
- 保持既有语义链路不破坏（未强制改写执行期自动触发策略）。

## 4. 验证结果

- 命令：`cargo test -q`
- 结果：全绿（0 失败）。

## 5. 风险状态更新

- R-002（GC 缺失风险）：从“方案空白”推进到“PoC 已落地，待集成增强”，状态保持 `In Progress`。
- 当前主要残余风险：
1. 自动触发时机与运行中触发安全性。
2. `HostPinRoot` 已最小实现，但仍需完善长期句柄治理与观测。
3. 大规模对象图下的标记/清扫性能与可观测性。

## 6. 下一轮 backlog（建议优先级）

1. 将 `ObjectId(slot+generation)` 安全语义扩展到更大规模 fuzz/压力回归（中优先级）。
2. 将运行中安全点触发从“最小模式”扩展到更细粒度策略（低优先级，先受控灰度）。

## 7. 进入条件（下一里程碑）

- GC 自动触发从“执行边界”扩展到“运行中可控触发”且不回退现有回归。
- GC fixture 与现有 test-harness 稳定运行。
- 风险 R-002 可转为“Low/监控中”再考虑 Phase 4 完全收尾。
