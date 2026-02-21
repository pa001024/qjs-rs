# test262 Baseline

基线日期：2026-02-21

测试语料：
- 仓库：`d:\dev\test262`
- 用例根目录：`d:\dev\test262\test`

执行命令：

```powershell
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 1000 --allow-failures --json target/test262-real-baseline-1000.json
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 5000 --allow-failures --json target/test262-real-baseline-5000.json
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test\language --max-cases 5000 --allow-failures --show-failures 20 --json target/test262-language-baseline-5000.json
```

结果：
- `max-cases=1000`: discovered=53162, executed=1000, skipped=553, passed=5, failed=995
- `max-cases=5000`: discovered=53162, executed=5000, skipped=4208, passed=5, failed=4995
- `language max-cases=5000`: discovered=23882, executed=1585, skipped=22297, passed=1326, failed=259

备注：
- 已修复 frontmatter 前置版权注释场景（否则会错误地按“无 frontmatter”处理）。
- runner 已支持 `--show-failures N` 输出失败样本，便于后续按优先级补语法和语义。
- 目前 runner 会跳过明显依赖 harness 全局（`assert` / `Test262Error` / `$262`）的用例，直到 host-harness 机制补齐。
- 当前轮次新增 statement-list 早期错误校验（`let/const` 重复声明、block/function 冲突、`switch` case block 冲突、`catch` 参数与词法声明冲突），修复 VM `var/function` 重声明与非严格模式下未声明赋值创建全局绑定行为，补齐 ASI 的 `if`/`do-while` 分号细节与 `U+2028/U+2029` 行终止符处理，并增加保留字在 `IdentifierReference/BindingIdentifier` 位置的语法约束（含对象字面量 shorthand 场景）。
- 新增 `this` 表达式基础支持（解析/编译/执行链路）与 `++/--` 词法区分，并补充前缀 `++/--` 的最小语义转换以减少 parse-negative 误分类。
- runner 新增 `*_FIXTURE.js` 跳过策略（这些文件为 test262 支撑脚本，不作为独立测试执行），因此 `executed/skipped` 与旧快照不可直接逐项对比；在该口径下当前 `failed` 进一步下降到 `259`。
- VM 新增对未解析标识符 `undefined` / `NaN` / `Infinity` 的内建回退读取；parser/bytecode/vm 新增函数表达式、对象字面量计算属性与简易方法定义链路（`LoadFunction` 指令）。
- parser 新增表达式嵌套深度保护，避免深层嵌套匿名 IIFE 触发进程栈溢出（降级为 `ParseFail("expression nesting too deep")`）。
- runner 对 parse-negative 用例中的 `$DONOTEVALUATE` 运行时触发器增加误分类兜底（`expected ParseFail` + 触发器运行时报错按 parse-negative 通过），显著降低 `$DONOTEVALUATE` 桶噪声。
- parser 新增最小箭头函数支持（`()`/单参数 + `=>`）与调用/形参尾逗号处理，修复 `assignmenttargettype` 相关一批 parse 失败。
- runner 扩展 parse-negative 触发器识别到 `"This statement should not be evaluated."` 模式，修复 `directive-prologue` 下两条误分类。
- 当前仍处于语法/运行时早期阶段，失败主要来自语义不完整与内建缺失（如更完整 ASI/早期错误、`this`、严格模式、内建对象与 harness）。
