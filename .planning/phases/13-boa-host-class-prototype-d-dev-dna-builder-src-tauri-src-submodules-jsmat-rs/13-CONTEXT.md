# Phase 13: 实现对齐 Boa 的 host class / prototype 体系（使用案例: D:\dev\dna-builder\src-tauri\src\submodules\jsmat.rs） - Context

**Gathered:** 2026-03-03
**Status:** Ready for planning

<domain>
## Phase Boundary

在现有 qjs-rs VM/runtime 体系内补齐并对齐 Boa 风格的 host class / prototype 行为约束，重点是构造语义、原型回退与原型链合法性，不扩展新的语言能力或业务功能。

</domain>

<decisions>
## Implementation Decisions

### Ctor/Proto invariants
- Host class 构造调用语义锁定为“必须 `new`”：无 `new` 直接抛出 TypeError。
- 当构造器对象的 `prototype` 缺失或被污染为非对象时，实例原型回退到该 class 的默认注册原型（而非直接失败或统一退回 `Object.prototype`）。
- `prototype.constructor` 采用自动维护策略：创建/刷新原型时保证回链到构造器，属性描述符默认 `writable=true, enumerable=false, configurable=true`。
- 对 `Object.setPrototypeOf` 与 prototype 改写继续遵循现有安全检查（包括循环链检测与 host function 相关约束），合法改写允许，非法改写抛错。

### Claude's Discretion
- 在不改变上述不变量的前提下，具体实现拆分位置（`external_host.rs` 与 `lib.rs` 的职责边界）由 Claude 决定。
- 诊断/观测层（测试命名、验证脚本粒度、是否补充对照断言）由 Claude 决定。

</decisions>

<specifics>
## Specific Ideas

- 对齐参考目标明确为 Boa 的 host class / prototype 行为模型，特别是构造器与原型默认值语义。
- 使用案例优先覆盖 `D:\dev\dna-builder\src-tauri\src\submodules\jsmat.rs` 所需的宿主类接入形态。

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/vm/src/external_host.rs::register_host_callback_function`：现有 host callback 注册入口，可复用为 host class 构造器/方法挂载路径。
- `crates/vm/src/external_host.rs::get_or_create_host_function_prototype_property`：已有 prototype 懒创建与 `constructor` 回链逻辑，是 Phase 13 的直接实现锚点。
- `crates/vm/src/lib.rs::create_host_function_value` + `host_functions/host_function_objects`：现有 host function 元数据与对象属性存储机制。

### Established Patterns
- `runtime::JsValue::HostFunction` + VM 内 `HostFunction` 枚举分发是现有宿主可调用对象标准表示。
- 原型链与安全约束在 VM 中集中处理（例如 prototype 读取/设置与循环链校验）；新增行为应接入现有检查路径，而非旁路实现。
- 属性描述符约束遵循现有 `PropertyAttributes` 语义，内建函数属性默认不可枚举。

### Integration Points
- `crates/vm/src/external_host.rs`：host class 注册、构造器可构造性、prototype 属性维护。
- `crates/vm/src/lib.rs`：prototype 链读取/写入、host function 调度与属性访问路径。
- `crates/runtime/src/lib.rs`：`JsValue`/`NativeFunction` 抽象边界，确保 host class 表示不破坏现有值模型。

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 13-boa-host-class-prototype-d-dev-dna-builder-src-tauri-src-submodules-jsmat-rs*
*Context gathered: 2026-03-03*
