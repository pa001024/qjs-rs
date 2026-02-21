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

CLI 参数：
- `--root <path>`: test262 或 test262-lite 根目录（必填）
- `--max-cases N`: 限制本次执行数量，便于快速迭代
- `--fail-fast`: 首次不匹配时立即返回错误
- `--allow-failures`: 即使存在失败也返回 0，适用于基线统计
- `--json <path>`: 将统计结果写入 JSON 文件
- `--show-failures N`: 输出前 N 条失败样本（路径+期望+实际）

后续计划：
- 接入真实 test262 仓库目录并解析更多 frontmatter 字段
- 支持更完整 `flags` / strict mode / include harness 机制
- 产出阶段性通过率报告
