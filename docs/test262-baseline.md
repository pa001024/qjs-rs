# test262 Baseline

基线日期：2026-02-21

测试语料：
- 仓库：`d:\dev\test262`
- 用例根目录：`d:\dev\test262\test`

执行命令：

```powershell
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 1000 --allow-failures --json target/test262-real-baseline-1000.json
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 5000 --allow-failures --json target/test262-real-baseline-5000.json
```

结果：
- `max-cases=1000`: discovered=53162, executed=1000, skipped=553, passed=5, failed=995
- `max-cases=5000`: discovered=53162, executed=5000, skipped=4208, passed=5, failed=4995

备注：
- 已修复 frontmatter 前置版权注释场景（否则会错误地按“无 frontmatter”处理）。
- 当前仍处于语法/运行时早期阶段，失败主要来自语法缺失与语义不完整（如 `var`、数组、对象高级语义、严格模式、内建对象与 harness）。
