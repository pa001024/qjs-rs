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
- `language max-cases=5000`: discovered=23882, executed=1585, skipped=22297, passed=1257, failed=328

备注：
- 已修复 frontmatter 前置版权注释场景（否则会错误地按“无 frontmatter”处理）。
- runner 已支持 `--show-failures N` 输出失败样本，便于后续按优先级补语法和语义。
- 目前 runner 会跳过明显依赖 harness 全局（`assert` / `Test262Error` / `$262`）的用例，直到 host-harness 机制补齐。
- 当前轮次新增 statement-list 早期错误校验（`let/const` 重复声明、block/function 冲突、`switch` case block 冲突、`catch` 参数与词法声明冲突），修复 VM `var/function` 重声明与非严格模式下未声明赋值创建全局绑定行为，补齐 ASI 的 `if`/`do-while` 分号细节与 `U+2028/U+2029` 行终止符处理，并增加保留字在 `IdentifierReference/BindingIdentifier` 位置的语法约束（含对象字面量 shorthand 场景）。
- runner 新增 `*_FIXTURE.js` 跳过策略（这些文件为 test262 支撑脚本，不作为独立测试执行），因此 `executed/skipped` 与旧快照不可直接逐项对比；在该口径下当前 `failed` 显著下降到 `328`。
- 当前仍处于语法/运行时早期阶段，失败主要来自语义不完整与内建缺失（如更完整 ASI/早期错误、`this`、严格模式、内建对象与 harness）。
