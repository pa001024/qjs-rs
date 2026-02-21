# test262 Baseline

基线日期：2026-02-21

测试语料：
- 仓库：`d:\dev\test262`
- 用例根目录：`d:\dev\test262\test`

执行命令：

```powershell
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 1000 --allow-failures --json target/test262-real-baseline-1000.json
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 5000 --allow-failures --json target/test262-real-baseline-5000.json
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test\language --max-cases 5000 --allow-failures --show-failures 10 --json target/test262-language-baseline-5000.json
```

结果：
- `max-cases=1000`: discovered=53162, executed=1000, skipped=553, passed=5, failed=995
- `max-cases=5000`: discovered=53162, executed=5000, skipped=4208, passed=5, failed=4995
- `language max-cases=5000`: discovered=23882, executed=1837, skipped=22045, passed=1159, failed=678

备注：
- 已修复 frontmatter 前置版权注释场景（否则会错误地按“无 frontmatter”处理）。
- runner 已支持 `--show-failures N` 输出失败样本，便于后续按优先级补语法和语义。
- 目前 runner 会跳过明显依赖 harness 全局（`assert` / `Test262Error` / `$262`）的用例，直到 host-harness 机制补齐。
- 当前轮次新增 statement-list 早期错误校验（`let/const` 重复声明、与 `var/function` 冲突、`switch` case block 冲突、`catch` 参数与词法声明冲突），`language` 基线净增 `+47` 通过。
- 当前仍处于语法/运行时早期阶段，失败主要来自语义不完整与内建缺失（如更完整 ASI/早期错误、`this`、严格模式、内建对象与 harness）。
