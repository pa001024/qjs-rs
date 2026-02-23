# test262-lite

`test262-lite` 是一个最小化的兼容性回归集，用于在完整 test262 接入前持续验证执行链路。

位置：
- 用例目录：`crates/test-harness/fixtures/test262-lite`
- 跑批测试：`crates/test-harness/tests/test262_lite.rs`

判定规则：
- 主要依据 frontmatter：`negative.phase` (`parse` / `runtime`)。
- 当前仍保留目录分类（`pass/`、`fail/parse/`、`fail/runtime/`）用于组织用例，但执行期望由 frontmatter 驱动。
- 若缺少 frontmatter `negative`，默认视为应通过。

执行方式：
- `cargo test -p test-harness test262_lite`
- `cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite`
- `cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --allow-failures`
- `cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --json target/test262-summary.json`
- `cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1`
- `cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1`
- `cargo run -p test-harness --bin test262-run -- --root crates/test-harness/fixtures/test262-lite --auto-gc --auto-gc-threshold 1 --runtime-gc --runtime-gc-interval 1 --show-gc`

GC 压测集成（当前）：
- 已新增 `gc-*` 样例：
  - `gc-closure-capture.js`
  - `gc-try-catch-finally.js`
  - `gc-with-scope.js`
  - `gc-nested-closure-chain.js`
  - `gc-cycle-drop.js`
  - `gc-looped-closures.js`
  - `gc-deep-object-chain.js`
  - `gc-array-object-churn.js`
  - `gc-exception-closure-interleave.js`
  - `gc-closure-chain-accumulator.js`
  - `gc-looped-record-frame.js`
  - `gc-cycle-triangle.js`
  - `gc-array-ring-buffer.js`
  - `gc-exception-nested-finally.js`
  - `gc-closure-bucket-rotation.js`
  - `gc-linked-list-rewrite.js`
  - `gc-runtime-array-burst.js`
  - `gc-linked-stack-rotation.js`
- `crates/test-harness/tests/test262_lite.rs` 运行套件时默认启用 `auto_gc=true` 且阈值 `1`，用于执行边界 GC 压测。
- 在 CLI 模式下以 `--auto-gc --runtime-gc` 跑批通过（当前样例：discovered=26, passed=26）。
- 最新 stress 快照：`collections_total=29283`、`boundary_collections=22`、`runtime_collections=29261`、`reclaimed_objects=611`。
- 在默认模式（不启用 `--auto-gc/--runtime-gc`）下，`--show-gc` 快照为全 0（`collections_total=0`、`boundary_collections=0`、`runtime_collections=0`）。
- `gc-array-object-churn.js` 现用于覆盖“嵌套调用 + runtime GC”场景，防止 caller stack roots 误回收回归。
- 快照对比报告见 `docs/gc-snapshot-report.md`。
- CLI 已支持输出套件级 GC 聚合统计（`--show-gc`）：
  - `collections_total`
  - `boundary_collections`
  - `runtime_collections`
  - `reclaimed_objects`
  - `mark_duration_ns` / `sweep_duration_ns`
- 句柄安全（`ObjectId slot+generation`）由 `crates/vm/src/lib.rs` 的 GC 单测覆盖，确保回收复用后 stale handle 不会误命中新对象。

CLI 参数：
- `--root <path>`: test262 或 test262-lite 根目录（必填）
- `--max-cases N`: 限制本次执行数量，便于快速迭代
- `--fail-fast`: 首次不匹配时立即返回错误
- `--allow-failures`: 即使存在失败也返回 0，适用于基线统计
- `--json <path>`: 将统计结果写入 JSON 文件
- `--show-failures N`: 输出前 N 条失败样本（路径+期望+实际）
- `--auto-gc`: 启用自动 GC（执行边界触发）
- `--auto-gc-threshold N`: 自动 GC 对象阈值（与 `--auto-gc` 搭配）
- `--runtime-gc`: 启用运行中安全点 GC 检查（需搭配 `--auto-gc`）
- `--runtime-gc-interval N`: 运行中 GC 检查间隔（opcode 计数）
- `--show-gc`: 输出 test suite 聚合 GC 统计
- `--expect-gc-baseline <path>`: 从基线文件加载 GC guard 阈值（`key=value`），可与以下 `--expect-*` 参数叠加（显式参数优先）
- GC guard 默认额外校验统计平衡：`collections_total == runtime_collections + boundary_collections`
- `--expect-collections-total-min N`: 断言 `collections_total >= N`（不满足时返回非 0）
- `--expect-runtime-collections-min N`: 断言 `runtime_collections >= N`（不满足时返回非 0）
- `--expect-runtime-ratio-min R`: 断言 `runtime_collections / collections_total >= R`（`R` 取值 `0.0..=1.0`）
- `--expect-reclaimed-objects-min N`: 断言 `reclaimed_objects >= N`（不满足时返回非 0）

基线文件示例（`crates/test-harness/fixtures/test262-lite/gc-guard.baseline`）：

```text
collections_total_min=1000
runtime_collections_min=1000
runtime_ratio_min=0.90
reclaimed_objects_min=1
```

后续计划：
- 接入真实 test262 仓库目录并解析更多 frontmatter 字段
- 支持更完整 `flags` / strict mode / include harness 机制
- 产出阶段性通过率报告
