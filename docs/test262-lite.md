# test262-lite

`test262-lite` 是一个最小化的兼容性回归集，用于在完整 test262 接入前持续验证执行链路。

位置：
- 用例目录：`crates/test-harness/fixtures/test262-lite`
- 跑批测试：`crates/test-harness/tests/test262_lite.rs`

分类约定：
- `pass/`: 必须成功执行
- `fail/parse/`: 解析阶段必须失败
- `fail/runtime/`: 运行阶段必须失败

执行方式：
- `cargo test -p test-harness test262_lite`

后续计划：
- 接入真实 test262 frontmatter 解析
- 支持 `negative`/`flags`/`features` 过滤
- 产出阶段性通过率报告
