# Semantics Checklist

基线日期：2026-02-23  
状态口径：`Done` / `In Progress` / `Planned`

## 1) 语言核心

| 语义点 | 状态 | 当前结论 | 下一验收门槛 |
| --- | --- | --- | --- |
| 数值字面量与算术求值顺序 | In Progress | 常见算术/比较链路可运行并有回归测试。 | 补齐异常值、隐式转换边角与更多 test262 子集。 |
| 绑定规则（`var/let/const`、TDZ） | In Progress | 主路径可运行，含一批 TDZ 与声明冲突语义。 | 覆盖更多 eval/块级作用域/早期错误交叉场景。 |
| 函数声明、闭包、`this` 绑定 | In Progress | 函数调用、闭包引用与基础 `this` 语义已落地；`obj.m()`/`obj[k]()` 与 `with` 环境下标识符调用已支持 receiver/base-object 绑定。 | 对 strict/sloppy、arrow、构造调用差异继续收敛。 |
| 对象属性访问/赋值/删除 | In Progress | 常见读写/删除/计算属性与访问器基线已接通。 | 完整属性描述符、不可写/不可配置细节继续对齐。 |
| 原型链查找与写入限制 | In Progress | 已有原型链查找与部分写入限制语义。 | 补齐 `[[Set]]` 边界、更细粒度 descriptor 约束。 |

## 2) 控制流与异常

| 语义点 | 状态 | 当前结论 | 下一验收门槛 |
| --- | --- | --- | --- |
| `if/while/for/switch` | Done | 控制流主路径稳定，已覆盖大量回归。 | 保持回归稳定并清理残余 corner cases。 |
| `return/break/continue` completion | In Progress | 具备 baseline completion 行为，含标签控制流。 | 与 `try/finally`、循环 completion 组合场景继续验证。 |
| `throw` 与异常传播 | In Progress | 具备抛出/捕获/传播主路径。 | 对跨 chunk、handler 栈隔离继续压测。 |
| `try/catch/finally` | In Progress | finally completion 覆盖已增强。 | 继续对齐 spec completion records 的复杂分支。 |

## 3) 运行时模型

| 语义点 | 状态 | 当前结论 | 下一验收门槛 |
| --- | --- | --- | --- |
| `JsValue` 表示与稳定性 | In Progress | 现有表示可支撑主路径执行。 | 结合 GC 设计评审句柄/对象生命周期一致性。 |
| 全局/词法环境分离 | In Progress | 当前已有 `Realm` + 词法环境协作。 | 在 eval/with/strict 组合场景补齐边界。 |
| 对象生命周期与 GC Root 正确性 | In Progress | 已落地 mark-sweep、root 收集、自动/运行中触发开关与 host pin；已实现 `ObjectId(slot+generation)` 防 stale handle。 | 扩大压力回归与观测范围，验证长期运行稳定性。 |

## 4) 平台特性

| 语义点 | 状态 | 当前结论 | 下一验收门槛 |
| --- | --- | --- | --- |
| Promise 微任务队列 | Planned | 尚未实现。 | 定义 host API、任务队列语义与回归集。 |
| ES Module 实例化/执行顺序 | Planned | 尚未实现。 | 建立 module parse/instantiate/evaluate 完整链路。 |
| 内建对象基线 | In Progress | `Object/Function/Array/String/Number/Boolean/Math/Date` 已有部分可用路径。 | 补齐 `JSON`、`Error` 细节与规范行为一致性。 |

## 5) 当前质量闸门

- 工作区回归：`cargo test -q`（2026-02-23）全绿。
- 兼容性方向：继续以 `test262-lite` + 真实 test262 子集迭代压降失败簇。
- 新语义改动要求：
  - 至少 1 个正向用例。
  - 至少 1 个边界/异常用例。
