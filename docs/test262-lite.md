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

后续计划：
- 接入真实 test262 仓库目录并解析更多 frontmatter 字段
- 支持更完整 `flags` / strict mode / include harness 机制
- 产出阶段性通过率报告
