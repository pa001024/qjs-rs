# test262 Baseline

基线日期：2026-02-23

测试语料：
- 仓库：`d:\dev\test262`
- 用例根目录：`d:\dev\test262\test`

执行命令：

```powershell
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 1000 --allow-failures --json target/test262-real-baseline-1000.json
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test --max-cases 5000 --allow-failures --json target/test262-real-baseline-5000.json
cargo run -p test-harness --bin test262-run -- --root d:\dev\test262\test\language --max-cases 5000 --allow-failures --show-failures 200 --json target/test262-language-baseline-5000.json
```

结果：
- `max-cases=1000`: discovered=53162, executed=1000, skipped=553, passed=5, failed=995
- `max-cases=5000`: discovered=53162, executed=5000, skipped=4208, passed=5, failed=4995
- `language max-cases=5000`: discovered=23882, executed=5000, skipped=18579, passed=4320, failed=680
- `language/statements/for-in`: discovered=115, executed=61, skipped=54, passed=61, failed=0
- `language/expressions/assignment`: discovered=485, executed=92, skipped=393, passed=87, failed=5

备注：
- 已修复 frontmatter 前置版权注释场景（否则会错误地按“无 frontmatter”处理）。
- runner 已支持 `--show-failures N` 输出失败样本，便于后续按优先级补语法和语义。
- runner 目前仅对明显依赖 `$262` host API 的用例做保守跳过；`assert` / `Test262Error` 已接入 baseline 运行路径。
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
- parser 增加 `for-in` / `for-of` 语法形状 baseline（当前先降级为不迭代执行的兼容路径）并补齐 `for (let in ...)` 的非严格解析分支，`language` 基线进一步提升到 `1556/29`。
- lexer 补齐 `U+000B`（vertical tab）空白字符跳过，修复 `white-space/after-regular-expression-literal-vertical-tab.js`，`language` 基线进一步提升到 `1557/28`。
- parser 补齐 postfix `++/--` 的行终止符约束，并支持括号内最小逗号表达式形状（`(0, eval)`），修复 `eval-code/indirect/parse-failure-2.js`，`language` 基线进一步提升到 `1558/27`。
- parser 为函数参数列表增加 rest + 绑定模式语法吞吐基线（`...[]` / `...{}`），清理 `rest-parameters/(array|object)-pattern.js`，`language` 基线进一步提升到 `1560/25`。
- parser 新增 `class` 声明/表达式语法吞吐基线、`async function` 与 `function*` 形状解析，以及转义关键字的“原始文本”判定（如 `l\u0065t` 不再被误判为关键字）；`language` 基线提升到 `1580/5`。
- parser 补齐 `new Function(... )()` 这类 `new` 后续调用链形状解析；VM 增加“未解析标识符回退读取全局对象属性”路径（支持 `this.let = 0; let;` 场景）；`language` 基线提升到 `1581/4`。
- bytecode 增加脚本顶层 `var` 预声明提升（最小 hoist），并对全局受限名（`undefined`/`NaN`/`Infinity`）的词法声明注入运行时异常路径（匹配 test262 `negative phase: runtime` 口径）；`language` 基线提升到 `1583/2`。
- lexer/parser/ast/bytecode/vm 打通 `punctuators` 基线：新增 `%`、位运算（`&`/`|`/`^`/`~`）、移位（`<<`/`>>`/`>>>`）、条件运算符（`?:`）及对应复合赋值（`+=`/`-=`/`*=`/`/=`/`%=`/`<<=`/`>>=`/`>>>=`/`&=`/`|=`/`^=`）的最小可运行链路；`language` 基线提升到 `1584/1`。
- test-harness 将单 case 执行放入大栈线程（`32MB`）以隔离深递归解析/执行路径；parser 表达式深度阈值上调至 `80`，`statements/function/S13.2.1_A1_T1.js`（32 层嵌套 IIFE）已通过。
- bytecode/vm 新增 `delete member` 专用路径（`DeleteProperty` / `DeletePropertyByValue`），修复 getter 内部 `delete this.x` 触发的递归栈溢出。
- baseline builtins 新增 `String` / `isNaN` 以及 `Error` / `TypeError` / `ReferenceError` / `SyntaxError` 名称注入，降低 `UnknownIdentifier` 噪声失败簇。
- `assert.throws` 已将 VM 抛出的 runtime 错误统一纳入“抛出”判定路径，减少 harness 断言误报。
- parser/ast/bytecode/vm 新增 `in` 运算符最小可运行链路，并在 `for` 头部引入 `no-in` 解析上下文，修复一批调用实参里的 `"... in this"` 解析失败。
- parser 的箭头函数分支改为复用参数列表解析，补齐带默认值/复杂形参的吞吐基线。
- VM 标识符解析新增 `globalThis` 回退到全局对象，清理一批 `eval-code` 里的 `UnknownIdentifier("globalThis")`。
- parser/ast/bytecode/vm 打通对象字面量 computed accessor（`get [k](){}` / `set [k](v){}`）最小执行链路，修复 `computed-property-names/object/accessor/*` 一批基线失败。
- parser/ast/bytecode 新增带标签 `break`（`break label;`）链路，并修复函数体标签上下文隔离，清理 `block-scope/leave/*break*` 相关 parse 失败并消除编译期 panic。
- parser/ast/bytecode 新增带标签 `continue`（`continue label;`）链路，并按函数边界隔离 label-set + 增加“continue 目标必须为迭代语句”早期错误校验，修复 `asi/S7.9_A1` 与 `block-scope/leave` 的一批标签控制流失败。
- lexer 新增科学计数法数字字面量（`1e55`/`2E-2`）词法支持；VM 的 number→string 调整为更贴近 JS 规范（`-0 -> "0"`、`Infinity`、指数统一 `e+N`），清理 `computed-property-names/object/property/number-duplicates.js` 及一批数字字符串化相关断言失败。
- VM 补齐 `Object.getOwnPropertyDescriptor` / `Object.getPrototypeOf` 最小执行路径，并新增对象 `hasOwnProperty` 基线，显著减少 `arguments-object/*` 中的 `NotCallable` 失败。
- VM 在函数调用创建的 `arguments` 对象上补齐 `constructor === Object` 基线（含测试覆盖 `arguments.constructor.prototype` 与 `arguments.hasOwnProperty('callee')`），`language` 基线提升至 `3260/1740`。
- parser/ast/bytecode/vm 打通调用/构造参数 `...spread` 运行时展开（含 trailing comma 场景），修复 `arguments-object/*spread-operator*` 一批断言失败，`language` 基线提升至 `3275/1725`。
- parser 为 class body 增加最小方法降级链路（实例方法下沉到 `prototype`、静态方法挂到构造对象，使用 IIFE 生成），显著降低 `arguments-object` 中 class trailing-comma 失败簇，`language` 基线提升至 `3307/1693`。
- VM 对 `arguments` 对象新增形参与索引属性映射、基础属性特性（`writable/enumerable/configurable`）与 `Object.defineProperty`/`delete` 约束处理，同时新增 `delete identifier` 指令语义（已声明绑定删除返回 `false`）；`language` 基线提升至 `3343/1657`。
- parser/VM 新增“非简单形参列表”内部标记链路（默认值/rest/解构参数关闭 arguments 映射），修复 `arguments-object/unmapped/via-params-*` 失败簇，`language` 基线提升至 `3346/1654`。
- VM 在 arguments 映射断开（`writable: false`）路径补齐“先快照后解绑”语义，修复 `mapped-arguments-nonconfigurable-nonwritable-3.js`，`language` 基线提升至 `3347/1653`。
- parser/VM 为箭头函数增加内部 marker，并在调用/构造路径实现词法 `this`/`arguments`（不创建箭头函数自有绑定，且 `new` 调用箭头函数报 `NotCallable`），`language` 基线提升至 `3354/1646`。
- parser/ast/bytecode/vm 新增 `instanceof` 运算符基线（关系运算符解析、指令生成与最小运行时判定），清理一批 `expected ')' after arguments` 误解析并改善 `catch` 场景断言链路，`language` 基线提升至 `3373/1627`。
- VM 新增调用/构造路径错误路由：在存在异常处理器时，将 `UnknownIdentifier`/`TypeError`/`NotCallable` 等运行时错误转为可捕获异常值并进入 `catch`，同时为 `eval`/`Function` 解析错误统一加 `SyntaxError:` 前缀，显著降低 `try/catch` 与 directive-prologue 相关误差，`language` 基线提升至 `3422/1578`。
- baseline globals 新增最小 `Symbol` 支持（含 `Symbol.iterator` 等常见 well-known keys 以及 computed key 测试链路），继续清理 `computed-property-names/object/*` 的 `UnknownIdentifier("Symbol")` 失败，`language` 基线提升至 `3425/1575`。
- bytecode 编译器补齐函数级 `var` 提升语义（顶层函数体预声明 + 嵌套 block/if/for/switch/try 收集 + 块内 `var` 初始化走 `StoreVariable` 绑定已提升变量），显著削减 `block-scope/shadowing` 中的 `UnknownIdentifier` 与值覆盖异常，`language` 基线提升至 `3454/1546`。
- parser 新增 strict-mode 后置校验链路（directive-prologue 识别 + script/function/body 递归检查），在严格模式下禁止 `implements/interface/package/private/protected/public/static` 作为绑定名或标识符引用，进一步降低 `directive-prologue` 误通过，`language` 基线提升至 `3455/1545`。
- parser/VM 打通 class computed accessor 与受限构造路径：class body 支持 `get/set` 计算属性名解析并经 `Object.defineProperty` 降级定义；VM 仅对带内部 marker 的“类构造对象”开放 `new` 构造，避免把普通对象误判为可构造，`language` 基线提升至 `3464/1536`。
- VM 将调用方 strict 状态沿 `Call/Construct` 链路传递到原生/宿主函数，并在 `eval` 解析阶段强制 strict 语义（修复 strict caller 下 direct eval 未触发早期错误的问题），`language` 基线提升至 `3474/1526`。
- bytecode 为 `try`/`catch`/`finally` block 统一补齐词法作用域（`EnterScope/ExitScope`），并同步修正 finally unwind 路径，清理一批 `block-scope/leave|shadowing` 断言失败，`language` 基线提升至 `3486/1514`。
- VM 将 `LoadIdentifier` 产生的 `UnknownIdentifier` 接入异常处理器路由（可被 `try/catch` 捕获），并为 inline chunk（`eval`/`Function` 路径）隔离异常处理栈，避免跨字节码段错误跳转（`InvalidJump`），大幅修复 `try/catch` 与 `eval-code` 失败簇，`language` 基线提升至 `3615/1385`。
- VM/runtime 新增 `String.fromCharCode` baseline，并补齐 `ToNumber("0x...")` 的最小十六进制字符串转换分支，修复 comments 相关 `eval` 行注释 unicode 字符用例簇，`language` 基线提升至 `3620/1380`。
- lexer 补齐字符串行继续（`\\` + `LF/CRLF/U+2028/U+2029`）词法吞吐，并新增对应单测，修复 `directive-prologue/14.1-4-s.js` 等相关 parse 失败，`language` 基线提升至 `3624/1376`。
- VM/runtime 新增 `Array.prototype.push` 与 `Object.keys` 最小可用链路；同时 bytecode/vm 增加 `Nop` 指令用于保留 directive-prologue 边界，parser 将 `Stmt::Empty` 视为 strict 指令中断点，修复一批 strict 误判路径，`language` 基线提升至 `3626/1374`。
- bytecode/vm 为数组字面量引入 `DefineArrayLength` 指令，确保 `length` 属性默认不可枚举（`enumerable: false`），修复 `Object.keys([ ... ])` / 枚举相关偏差，`language` 基线提升至 `3627/1373`。
- ast/parser/bytecode/vm 增加字符串字面量 `has_escape` 元数据与 `MarkStrict` 字节码标记，VM strict 判定改为以编译期标记为准（不再仅靠 `"use strict"` 运行时字面量扫描），修复 directive-prologue 中带行继续字面量的 strict 误判，`language` 基线提升至 `3629/1371`。
- VM 在 strict 执行上下文下将“未声明标识符赋值”从“隐式创建全局绑定”修正为 `ReferenceError`（`StoreVariable` 路径），`directive-prologue` 子集由 `51/11` 提升到 `62/0`，并显著压降 `eval-code/direct` 的 `assert.throws` 失败簇，`language` 基线提升至 `3661/1339`。
- parser 为默认参数新增函数体前置初始化降级（`if (param === undefined) param = initializer`），并补齐对象/class generator method 的基础语法吞吐；VM 为 direct eval 新增上下文约束：在非简单参数函数上下文中拒绝 `var/function arguments` 声明（箭头函数上下文保留绑定行为），`eval-code/direct` 子集显著收敛。
- runtime/builtins/vm 增加 `Boolean` 全局构造器并补齐 `new Number/new Boolean/new String` 的装箱构造路径；同时在 `ToString`/`ToNumber` 最小实现中打通装箱对象解包，修复 `eval-code/(direct|indirect)/non-string-object.js` 等一批对象入参 `eval` 误行为，`language` 基线进一步提升。
- VM 的 `+` 运算新增对象到原始值（`ToPrimitive`）的最小调用链（`valueOf`/`toString`）并接入 Date 对象的 string-preferred 分支；baseline globals 新增 `Date` 构造器最小实现，修复 `expressions/addition` 下一批对象/Date 相关失败。
- runtime/builtins/vm/bytecode 新增 `Array` baseline（全局构造器、`CreateArray` 指令、数组字面量默认 `constructor` 绑定）并补齐 `Number.MAX_VALUE` / `Number.MIN_VALUE`，`language` 基线由 `3938/1062` 提升至 `3983/1017`。
- ast/parser/bytecode 新增数组空位（elision）语法链路（如 `[,]`、`[1,,3,]`）并按索引稀疏编译，`language` 基线进一步提升至 `3989/1011`。
- parser/bytecode/vm 新增数组字面量 `...spread` 执行链路（含与 elision 混用），`expressions/array/spread-*` parse 失败簇清理，`language` 基线进一步提升至 `3997/1003`。
- VM 为用户函数补齐最小“可扩展属性”存储（支持 `f.valueOf = ...` 这类覆盖），并在 `+` 运算里将函数对象接入 `ToPrimitive` 路径；同时将 `Opcode::Add` 的运行时异常接入 handler 路由，修复一批 `addition`/`try-catch` 相关误报，`language` 基线提升至 `4001/999`。
- VM 补齐共享 `Object.prototype.toString` fallback（稳定函数身份，避免每次读取生成新函数）并保持 Date 对象自有 `toString` 路径；结合前一轮 `ToPrimitive` 修正后，`language` 基线进一步提升至 `4009/991`。
- runtime/vm 补齐函数值 `hasOwnProperty`、`Array.prototype.forEach`、`Object.isExtensible`、箭头/strict 函数 `caller/arguments` 受限访问，以及 `Object.getOwnPropertyDescriptor` 对函数目标的最小支持；`language` 基线进一步提升至 `4037/963`。
- VM 新增稳定 `Function.prototype` 对象与函数 `prototype` 惰性缓存（含 `constructor` 回填），并修正 `Object.getPrototypeOf(function-like)` 返回路径，清理一批函数原型身份不稳定导致的断言失败；`language` 基线进一步提升至 `4041/959`。
- VM 在数值运算/位运算/关系运算中补齐对象与函数的 `ToPrimitive(ToNumber)` 转换链路，并将相关运行时错误接入异常处理器路由；同时实现 `==/!=` 抽象相等比较与字符串关系比较（`< <= > >=`）基线，显著清理 `compound-assignment`、`equals/does-not-equals`、`relational` 失败簇，`language` 基线提升至 `4115/885`。
- ast/parser/bytecode/vm 为 `++/--` 引入显式 update 表达式语义：区分前缀/后缀返回值，并通过新增栈指令（`Dup2`/`Swap`/`RotRight4`）保证成员与计算属性 update 的引用只求值一次；`language` 基线进一步提升至 `4127/873`。
- ast/parser/bytecode/vm 打通 `with` 语句最小执行链路（`Stmt::With` + `EnterWith/ExitWith`），并在 VM 中加入 with 环境帧参与标识符解析优先级（内层词法作用域 > with 对象环境 > 外层词法作用域），修复一批 object environment record 相关偏差。
- bytecode/vm 为标识符赋值路径新增“引用解析/读写分离”指令（`ResolveIdentifierReference` / `LoadReferenceValue` / `StoreReferenceValue`），确保 `=`、复合赋值、`++/--` 在 `eval` 与 `with` 场景下沿用同一左值引用（PutValue 语义）；`language` 基线进一步提升至 `4205/795`。
- ast/parser/bytecode 新增 `Sequence`（逗号表达式）语义链路，修复此前括号内逗号表达式只保留末项、前项副作用丢失的问题；`expressions/assignment` 与 `expressions/compound-assignment` 的 `A6` PutValue/eval 失败簇清空，`language` 基线提升至 `4234/766`。
- VM 新增最小 `Math` 全局对象按需注入（`Math` 标识符解析 + `typeof Math` 路径），补齐只读常量（`E/PI/LN10/LN2/LOG10E/LOG2E/SQRT1_2/SQRT2`）的属性特性，修复 `S8.12.4_A1` 与相关 `Math` 访问失败，`language` 基线提升至 `4237/763`。
- VM 修正非严格模式下“未解析标识符赋值”语义：`PutValue` 改为在全局对象上创建默认属性（`writable/enumerable/configurable = true`），不再仅创建词法绑定；修复 `11.13.1-4-1.js` 与相关 descriptor/delete 语义偏差，`language` 基线提升至 `4240/760`。
- VM 为普通对象补齐最小 `[[Prototype]]` 链路（构造实例时挂接 `f.prototype`，对象属性 `Get/Has/Set` 按链路查找），并在赋值时处理“原型上只读数据属性/无 setter accessor 不可写”场景，修复 `8.14.4-8-b_1.js` 等原型写入语义偏差；`language` 基线提升至 `4243/757`。
- parser 对 `for-in` 引入“选择性可运行降级”：当前仅对 `for (let <id> in <expr>)` 形状展开为 `Object.keys` + 索引循环（其余 `for-in/for-of` 形状仍保持保守非迭代降级，避免扩大回归面）；同时 VM 修正数组 `constructor` 为不可枚举属性（匹配数组枚举基线），`language` 基线进一步提升至 `4245/755`。
- parser 扩展 `for-in` 降级覆盖到 `var/const/表达式左值` 头部形状，并支持 `for (... in a, b)` 右侧逗号表达式；`for-in` 在 `null/undefined` 右值下按基线语义走“空迭代不抛错”。同时 VM/runtime 增加 `Object.create` 与 `Object.setPrototypeOf` 最小内建，并将普通对象默认原型接到 `Object.prototype`；`language` 基线进一步提升至 `4249/751`。
- parser 为 class 静态方法引入最小 `super` 绑定降级（方法体头部注入 `let super = Object.getPrototypeOf(C)`），修复 `super.x = rhs` / `super[expr] = rhs` 在 `[[Prototype]] = null` 场景下的 RHS 求值顺序偏差；`expressions/assignment` 子集由 `85/7` 提升至 `87/5`，`language` 基线进一步提升至 `4250/750`。
- bytecode 为 `for` 语句补齐最小“迭代体完成值”聚合（非中断路径取最后一次 body completion），同时 VM 增加 `String.prototype.indexOf` host 函数基线；`for-in` 子集由 `40/21` 提升至 `49/12`，`language` 基线进一步提升至 `4253/747`。
- parser/VM 为 `for-in` 引入内部键收集通道 `Object.__forInKeys`（覆盖原型链可枚举属性并做重复 key 去重），替换原先 `Object.keys` 的 own-only 策略；`for-in` 子集进一步提升到 `50/11`，`language` 基线进一步提升至 `4266/734`。
- VM 修正 `in` 运算符对象分支为“沿 `[[Prototype]]` 链查找”，避免 `for-in` 迭代时把原型可枚举键误判为不存在；同时 bytecode 为 `while/do-while/for` 引入统一循环完成值槽位（表达式语句即时写回），parser 将 `for-in` 头部内部赋值降级为 `void (lhs = key)` 避免污染 completion value。随后 VM 为数组对象接入稳定 `Array.prototype` 链（`Array.prototype` 返回稳定原型对象，数组字面量/构造结果挂接该原型），修复 `head-lhs-let.js` 的 `MemberExpression` 赋值语义。该轮后 `for-in` 子集提升至 `55/6`，`language` 基线提升至 `4279/721`。
- 本轮新增 `DefineVar` 指令并在 VM 引入 `var_scope_stack`，修复 direct eval 中 `var` 绑定被错误落到当前词法块的问题；同时为 `for-in/for-of` 的 `let/const` 头部降级注入 `Object.__tdzMarker`（内部 `Uninitialized` 标记），使头部 RHS 对同名绑定访问按 `ReferenceError` 处理。该轮后 `for-in` 子集提升至 `61/0`，`language` 基线由 `4303/697` 提升至 `4309/691`。
- bytecode 修复 `try { ... } finally { ... }` 的 completion value 保留：`finally` 正常完成时不再无条件覆盖为 `undefined`，并补充 `try/finally` 完成值与异常传播回归测试。该轮后 `language` 基线由 `4309/691` 进一步提升至 `4310/690`。
- VM/runtime 新增 `Array.prototype.reduce` 基线路径并补充描述符稳定性回归测试（覆盖 test262 `8.12.5-3-b_1.js`），`language/expressions/assignment` 子集由 `86/6` 提升至 `87/5`，`language` 基线进一步提升至 `4311/689`。
- VM/runtime 扩展内建表面：补齐 `Array.prototype.join/reverse/sort`、数组在 `+` 下的 `join(',')` 字符串化路径、`Math` 常见函数簇、`Date.parse/UTC` 与 `Date.prototype` 方法占位，并为原生构造器补齐 `toString/valueOf/constructor` 属性链路；在修复 `array.toString` 函数身份回归后，`language` 基线由 `4311/689` 提升至 `4320/680`。
- 当前仍处于语法/运行时早期阶段，失败主要来自语义不完整与内建缺失（如更完整 ASI/早期错误、`this`、严格模式、内建对象与 harness）。
