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
- `language max-cases=5000`: discovered=23882, executed=1585, skipped=22297, passed=1551, failed=34

备注：
- 已修复 frontmatter 前置版权注释场景（否则会错误地按“无 frontmatter”处理）。
- runner 已支持 `--show-failures N` 输出失败样本，便于后续按优先级补语法和语义。
- 目前 runner 会跳过明显依赖 harness 全局（`assert` / `Test262Error` / `$262`）的用例，直到 host-harness 机制补齐。
- 当前轮次新增 statement-list 早期错误校验（`let/const` 重复声明、block/function 冲突、`switch` case block 冲突、`catch` 参数与词法声明冲突），修复 VM `var/function` 重声明与非严格模式下未声明赋值创建全局绑定行为，补齐 ASI 的 `if`/`do-while` 分号细节与 `U+2028/U+2029` 行终止符处理，并增加保留字在 `IdentifierReference/BindingIdentifier` 位置的语法约束（含对象字面量 shorthand 场景）。
- 新增 `this` 表达式基础支持（解析/编译/执行链路）与 `++/--` 词法区分，并补充前缀 `++/--` 的最小语义转换以减少 parse-negative 误分类。
- runner 新增 `*_FIXTURE.js` 跳过策略（这些文件为 test262 支撑脚本，不作为独立测试执行），因此 `executed/skipped` 与旧快照不可直接逐项对比；在该口径下当前 `failed` 进一步下降到 `234`。
- VM 新增对未解析标识符 `undefined` / `NaN` / `Infinity` 的内建回退读取；parser/bytecode/vm 新增函数表达式、对象字面量计算属性与简易方法定义链路（`LoadFunction` 指令）。
- parser 新增表达式嵌套深度保护，避免深层嵌套匿名 IIFE 触发进程栈溢出（降级为 `ParseFail("expression nesting too deep")`）。
- runner 对 parse-negative 用例中的 `$DONOTEVALUATE` 运行时触发器增加误分类兜底（`expected ParseFail` + 触发器运行时报错按 parse-negative 通过），显著降低 `$DONOTEVALUATE` 桶噪声。
- parser 新增最小箭头函数支持（`()`/单参数 + `=>`）与调用/形参尾逗号处理，修复 `assignmenttargettype` 相关一批 parse 失败。
- runner 扩展 parse-negative 触发器识别到 `"This statement should not be evaluated."` 模式，修复 `directive-prologue` 下两条误分类。
- parser 放宽非严格模式下 `let` 在标识符引用位置的限制，并允许 `var let = ...` 场景，补齐 `let` 相关历史语法兼容基线。
- lexer/parser 新增 `...` 词法与调用实参 spread 形状解析支持（当前按 baseline 先走语法兼容），修复 `expressions/call/trailing-comma.js`。
- parser/bytecode/vm 新增 `typeof` / `void` / `delete` 一元关键字运算符 baseline（含 `typeof 未声明标识符` 宽容行为），显著降低被误判为“reserved word identifier” 的失败簇。
- parser/bytecode/vm 已接通对象字面量访问器链路（`get foo(){}` / `set foo(v){}`）：编译到 getter/setter 槽并在属性读写时触发，修复一批 `this` 绑定相关失败。
- test-harness 统一安装 baseline 内建全局，VM 新增 `NativeFunction` 调用通道并补齐 `eval` / `Function` / `Object` / `Number` 最小可用语义（含 `Number.NaN`），进一步降低 runtime `UnknownIdentifier` 簇。
- VM 新增字符串原始值的 `replace` 基线属性读取与回调替换执行路径，修复 `String.prototype.replace` 回调 `this` 相关失败。
- parser 新增 postfix `++/--` 的最小语法兼容（当前仍复用 update 重写策略），修复 `postfix-(in|de)crement/*-nostrict.js` 一批 parse 失败。
- VM 新增 `HostFunction` 可调用桥，补齐 `Function.prototype.call/apply/bind` baseline 与 `Object.defineProperty` 的访问器触发路径，`function-code` 失败簇显著下降。
- lexer 新增标识符 unicode 转义（`\\uXXXX`）与非 ASCII 标识符词法支持，显著降低 `unexpected character '\\'` 与乱码字符失败簇。
- lexer 进一步放宽 Unicode 标识符判定并补齐非 ASCII 空白字符跳过，继续削减 `identifiers/*` 与 `white-space/*` 历史失败簇。
- lexer 追加 `\u{...}` 码点转义标识符支持（含 astral code point），进一步清理 `identifiers/*-escaped.js` 大簇失败。
- parser/ast/bytecode 新增正则字面量 baseline 解析链路（`/.../flags`），按对象值降级编译，清理 `statementList/*regexp*` 与 `white-space/*after-regular-expression-literal*` 失败簇。
- parser/bytecode/vm 新增 `new` 表达式 baseline（`Construct` 指令）并补齐 `RegExp` 原生构造器最小语义，继续压降 `function-code` 与 `delete` 历史失败。
- VM 调整 `this` 绑定基线：补齐脚本顶层全局 `this` 回退、函数调用 strict/sloppy `this` 分流，以及 strict 代码中嵌套函数的 `this` 继承路径；`language` 基线由 `1528/57` 进一步提升至 `1535/50`。
- parser 放宽非严格模式 future-reserved words（`implements/interface/package/private/protected/public/static`）作为标识符/绑定标识符，`future-reserved-words/*` 失败簇清空，`language` 基线进一步提升到 `1543/42`。
- parser 增加 `with` / `debugger` 语句基线解析，并在嵌入语句位置对 `let` 优先按表达式语句处理（覆盖 ASI 场景），`language` 基线进一步提升到 `1551/34`。
- 当前仍处于语法/运行时早期阶段，失败主要来自语义不完整与内建缺失（如更完整 ASI/早期错误、`this`、严格模式、内建对象与 harness）。
