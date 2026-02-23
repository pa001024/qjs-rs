# Risk Register

最后评审日期：2026-02-23

| ID | Risk | Impact | Mitigation | Status |
| --- | --- | --- | --- | --- |
| R-001 | QuickJS C 内部细节无法 1:1 映射 Rust 所有权模型。 | High | 优先语义一致；内部实现采用句柄/索引抽象，避免机械翻译。 | Open |
| R-002 | GC 行为不稳定导致对象驻留或误回收。 | Low | 已落地 mark-sweep、root 集合、auto/runtime 触发与 `ObjectId(slot+generation)` 句柄防护；Default/Stress profile 观测/触发校验闭环（Step 9 完成），回归满足 `gc_stats==default`（auto/runtime 关闭）、`boundary_collections==collections_total`/`runtime_collections==0`、`runtime_collections>0` 且 `collections_total==runtime+boundary`。证据：Default 命令 `test262-run --show-gc` 快照 `collections_total=0`、`boundary_collections=0`、`runtime_collections=0`；Stress 命令 `test262-lite --auto-gc --runtime-gc --auto-gc-threshold 1 --runtime-gc-interval 1` + `test262-run --show-gc` 快照 `collections_total=29283`、`boundary_collections=22`、`runtime_collections=29261`、`reclaimed_objects=611`。 | Monitoring |
| R-003 | 控制流与异常 completion 语义在复杂嵌套下回归风险高。 | High | 扩充 `try/finally + break/continue/return` 组合回归；保持失败簇追踪。 | In Progress |
| R-004 | `eval/with/strict` 交叉场景容易出现作用域偏差。 | High | 将该组合列为专门语义清单与 nightly 回归桶。 | In Progress |
| R-005 | 属性描述符与原型链写入限制尚未完全对齐规范。 | Medium | 增加 descriptor 专项用例，逐步补齐 `[[Set]]`/`[[DefineOwnProperty]]` 边界。 | In Progress |
| R-006 | builtins 覆盖不足造成“引擎语义正确但兼容性表现差”。 | Medium | 以失败簇驱动 builtins 扩面，先补核心对象与 Error/JSON。 | In Progress |
| R-007 | 文档状态与真实实现脱节，影响任务优先级判断。 | Medium | 每次里程碑后同步 `current-status`、mapping、checklist、risk 文档。 | In Progress |
| R-008 | test262 通过率改善受 host hooks 与 harness 缺口限制。 | Medium | 按阶段实现最小 host API，并记录“跳过口径”避免误读数据。 | In Progress |
| R-009 | 过早做性能优化会稀释语义正确性投入。 | Medium | 保持优先级：语义正确性 > 可维护性 > 性能；优化以基线数据驱动。 | Open |
| R-010 | 运行中 GC 在高频数组 churn 下可能暴露 `UnknownObject` 句柄稳定性缺陷。 | High | 已通过 `gc_shadow_roots` 修复 caller stack roots 丢失；新增 VM 回归 `runtime_gc_keeps_caller_stack_roots_across_nested_calls` 与 test262-lite `gc-array-object-churn.js` 持续监控。 | Monitoring |

## 当前高优先级风险闭环（下一轮）
1. R-002：已完成 Step 9 Default/Stress profile 触发+观测闭环，当前通过 default 快照（全 0）与 stress 快照（`collections_total=29283`、`boundary_collections=22`、`runtime_collections=29261`、`reclaimed_objects=611`）验证 `gc_stats` 关系，风险等级降至 Low/Monitoring，继续常规监控。
2. R-003/R-004：继续压测 completion 与作用域交叉语义。
3. R-005/R-006：通过 test262 失败簇反推 descriptor 与 builtins 优先级。
