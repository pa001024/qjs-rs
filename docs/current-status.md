# Current Status Snapshot

基线日期：2026-02-24

## 1. 复核范围

- 代码结构：workspace、crate 布局、CI 配置。
- 运行质量：`cargo test -q`。
- 规划对齐：Phase 0~7 当前状态与下一阶段缺口。

## 2. 关键结果

- 工作区结构完整：`crates/ast`、`crates/lexer`、`crates/parser`、`crates/bytecode`、`crates/vm`、`crates/runtime`、`crates/builtins`、`crates/test-harness`。
- CI 已存在并覆盖格式化/静态检查/测试：`.github/workflows/ci.yml`。
- CI 已接入 GC guard stress gate（`test262-run --expect-gc-baseline crates/test-harness/fixtures/test262-lite/gc-guard.baseline`），用于持续监控 runtime/reclaimed 统计回归。
- 本地复核 `cargo test -q` 全部通过（0 失败）。
- `test262 language --max-cases 5000` 最新快照：`passed=4908`、`failed=92`（命令见 `docs/test262-baseline.md`，快照：`target/test262-language-baseline-5000-20260224-v35.json`）。
- 本轮新增语义收敛：
  - `obj.m()` / `obj[k]()` 调用已通过 `CallMethod*` 保留 receiver 绑定。
  - 标识符调用新增 reference-aware 路径（`CallIdentifier*`），修复 `with (obj) { method(); }` 的 `this` 绑定。
  - `super` 运行时回退链路在对象方法场景可用（`{ __proto__: proto, m() { return super.x; } }`）。
  - baseline 内建补齐 `parseInt`、`parseFloat`、`isFinite`。
  - 字符串词法补齐：前导小数字面量、`\u{...}` code point 转义、`\uD800-\uDFFF` surrogate 转义最小支持。
  - VM 关系运算中的字符串比较改为按 UTF-16 code unit 顺序（与 JS 规范/QuickJS 行为方向一致）。
  - 数值词法补齐：十六进制字面量 `0x.../0X...`。
  - parser 在 `Expression` 上下文（statement / if-while-do / for 条件更新 / return / throw / switch / with）补齐逗号运算符序列解析，避免把合法 `a, b` 误判为语句分隔错误。
  - lexer 对齐 QuickJS 风格新增 `is_regexp_allowed` 路径：在允许正则的上下文将 `/.../flags` 作为单 token 扫描，修复 regexp 字面量中的 `\` 词法失败。
  - lexer/parser 新增 template literal 分段 token 与解析（含 cooked/raw 区分、line-continuation raw 保真、tagged template 最小调用降级）。
  - tagged template 首参已从“仅 cooked 数组”升级为“cooked 数组 + `raw` 数组属性”，并补齐 `new tag\`...\`` 优先级（tagged template 高于 `new` 构造解析）。
  - template invalid escape 场景在 tagged template 下不再 parse-fail，改为 cooked `undefined` + raw 保留，收敛 `tagged-template/invalid-escape-sequences.js`。
  - class lowering 改为始终生成函数构造器（含空 class），并将实例方法改为 `Object.defineProperty`（`enumerable: false`）定义。
  - VM 函数对象补齐“显式 `[[Prototype]]` 改写”状态：`Object.setPrototypeOf(fn, null)` 后不再错误回退到 `Function.prototype`。
  - `Object.defineProperty` 已支持函数闭包目标，函数属性访问/写入补齐 accessor 路径，修复 class static computed `constructor` getter/setter 失败簇。
  - 构造路径移除“实例强制写入自有 `constructor`”行为，恢复通过原型链解析 `constructor`，修复 `class { ['constructor']() {} }` 语义偏差。
  - bytecode 将 `var` 初始化改为 reference-aware PutValue 路径（`ResolveIdentifierReference + StoreReferenceValue`），修复 `with` 语句内 `var x = ...` 错误绕过对象环境的问题。
  - regex 运行时最小可用链路增强：regex literal 改为调用 `RegExp(pattern, flags)`、`RegExp` 对象补齐 `global/ignoreCase/multiline/unicode/sticky/dotAll/lastIndex` 属性，并新增 `test()` host 路径（Rust regex backend），收敛 `literals/regexp` 的 `NotCallable` 与 `instanceof` 失败。
  - class method/accessor 函数新增“不可构造、无 prototype”标记，VM 在 `new`/`in`/属性读取路径按该标记处理，进一步对齐 class 方法行为。
  - 函数 `length` 从“形参总数”修正为“首个默认参数前的形参数量”（含 class/object/arrow/function 默认参数场景），清理 `dflt-params-trailing-comma` 失败簇。
  - parser 新增可选 `catch` 绑定语法（`catch { ... }`），修复 `scope-catch-param-*` parse 失败簇。
  - bytecode 修复 `switch` 与 `try/catch` completion value 传播（保留分支最后求值结果，不再统一丢成 `undefined`），清理一批 `statements/(switch|try)/cptn-*` 失败。
  - class lowering 对齐 descriptor 细节：`C.prototype` 改为不可写/不可配/不可枚举，static method 统一经 `Object.defineProperty(enumerable:false)` 定义；同时 VM 跳过内部 class 临时名推断，修复 `class/definition` 中 `basics/methods/prototype-property` 失败。
  - bytecode 的 statement-list 最后取值目标改为跳过 `var/let/const/function/empty` 空完成值语句，并修复 `var` 初始化的栈残留（`StoreReferenceValue` 后补 `Pop`），进一步清理 `statements/{class,const,empty,let,variable}/cptn-*` 失败簇。
  - runtime/builtins 将 `Error/TypeError/ReferenceError/SyntaxError/EvalError/RangeError/URIError` 拆分为独立 Native constructor，避免全部错误落成 `Test262Error` 字符串前缀。
  - VM `instanceof` 收敛：错误构造器匹配从“泛 Error”改为按构造器名精确匹配；同时补齐 RHS `prototype` 非对象时的 TypeError 与对象左值原型链匹配。
  - VM `in`/`instanceof` 运行时异常已统一接入 handler 路由，可被 `try/catch` 捕获（不再直接顶层失败）。
  - String baseline 补齐 `String.prototype.split(separator, limit)` 最小可运行路径，并在字符串属性可见性里暴露 `split`。
  - `DefineVariable` 重声明写回策略收敛：`undefined` 仅对内部临时名（`$__loop_completion_`/`$__switch_tmp_`/`$__class_ctor_`）回写，避免污染用户 `var/function` 绑定。
  - 标识符引用回退路径补齐：`globalThis`/`Math`/`this`/realm globals/global object 属性可在 `Unresolvable` 路径读取，降低 `UnknownIdentifier` 噪声。
  - parser strict 校验对齐 QuickJS：`eval/arguments` 作为 strict 绑定名或赋值目标时抛 SyntaxError，并补齐 strict 函数重复形参早期错误（`13.1-23/25/27/29/31/33-s`）。
  - VM 对函数值 `caller/arguments` 限制扩展到 host/native function（含 `bind()` 产物），并将 `Get/SetProperty*` 的运行时错误纳入异常处理器路由，允许 `try/catch` 捕获属性访问 TypeError。
  - `language/statements/function` 子集从 `175/32` 提升到 `182/25`（executed=207）。
  - VM `eval` 调用链路拆分为 direct/indirect 两类语义路径：仅 direct `eval(...)` 继承 caller strict 语境，普通调用路径（如 `(0, eval)(...)`）按 indirect 规则执行。
  - `eval` 作用域策略对齐推进：strict eval 使用隔离变量环境；indirect eval 切换到全局执行上下文（清理 caller with 环境影响），non-strict 维持函数声明对 caller/global 的可见性。
  - eval 补齐 global function 可声明性守卫：当 eval 命中全局 var 环境且声明受限函数名（如 `NaN/Infinity/undefined`）时抛 TypeError，修复 `non-definable-global-function`。
  - `language/eval-code` 子集从 `162/18` 提升到 `168/12`（executed=180）。
  - parser class lowering 补齐 `extends` 链路：保留 extends 表达式、派生类默认构造器生成 `super(...arguments)`、方法 super 绑定改为基于 extends 值（覆盖 static/instance/constructor）。
  - VM 新增 `super(...)` 专用构造调用路径（不再走普通 Call），修复 derived constructor 调父类构造器时的 `class constructor cannot be invoked without 'new'` 误报。
  - bytecode 对 `super.method(...)` 与 `super[expr](...)` 调整 receiver 绑定，调用路径改为以当前 `this` 作为 thisArg。
  - `language/expressions/super` 子集从 `9/23` 提升到 `15/17`（executed=32）；`language/statements/class/subclass` 子集从 `17/60` 提升到 `22/55`（executed=77）。
  - lexer 字符串转义补齐 legacy 路径：支持 legacy non-octal（如 `\8`、`\9`、`\A`、`\Ð`）与 legacy octal（如 `\1`/`\\123`）最小吞吐。
  - `language/literals/string` 子集提升到 `59/0`（executed=59）。
  - parser `new` 表达式对齐 QuickJS 右递归语义：`new NewExpression` 递归解析，修复 `new new Boolean(true)` 等历史 parse-fail。
  - bytecode/vm 增加 `super` 专用 opcode（`Get/SetSuperProperty*`、`PrepareSuperMethod*`）与 `Dup3/RotRight5` 栈操作，修复 `super.prop`/`super[expr]` 的 `this` 绑定、key 求值顺序与写入路径。
  - VM 增加 runtime `ToPropertyKey` 调用链（含对象 `toString` 副作用与异常传播），修复 `super[badToString]` 与 `GetSuperBase before ToPropertyKey` 相关失败簇。
  - direct eval 增加 `parse_script_with_super` 通道：在有 super 语境时放行 `eval('super.x')` 解析。
  - VM 增加 `Object.freeze` 最小语义（对象 `extensible=false`）与 `String.prototype.toLowerCase`，并修复 `hasOwnProperty.call(...)` 的 this 覆盖语义。
  - runtime 错误路由改为 Error-like 对象（`constructor/name/message`），`TypeError`/`ReferenceError` 不再仅以字符串抛出。
  - `language/expressions/super` 子集从 `15/17` 提升至 `32/0`（executed=32）。
  - parser 补齐“箭头函数默认参数=非简单参数”内部 marker（`$__qjs_non_simple_params__$`），修复 `EnterParamInitScope` 误弹参数作用域导致的 `UnknownIdentifier("p")` 回归。
  - `language/eval-code/direct` 子集提升至 `143/0`（executed=143）。
  - VM `code_has_marker` 改为全字节码扫描，修复 class method 注入 `use strict/let super` 前导后 marker 丢失导致的 non-simple 参数误判。
  - VM 新增 primitive boxing 路径（非严格函数 `this` + `GetProperty` on number/bool），并补齐函数值 `constructor` 回退属性，修复 `function-code` 的 `10.4.3` / `S10.2.1` 失败簇。
  - parser 为对象参数模式增加最小副作用降级（computed key 与属性默认值 initializer），修复 `eval-param-env-with-*` 两条 function-code 剩余失败。
  - `language/function-code` 子集提升至 `173/0`（executed=173）。
  - bytecode 将 unary plus 降级改为 `ToNumber` 语义（`expr - 0`），并在 VM 数值转换路径收紧 `Infinity` 大小写匹配，`language/expressions/unary-plus` 收敛至 `16/0`。
  - VM 补齐 delete 关键语义：`null/undefined` base 抛 TypeError、`delete super[...]` 抛 ReferenceError（且不触发 `ToPropertyKey`）、全局常量属性（`NaN/Infinity/undefined`）不可配置、全局 `var` 与 `globalThis` 属性联动。
  - VM 新增 `JSON` 最小对象（`stringify/parse`）与 `Array.isArray`，`language/expressions/delete` 收敛至 `56/0`。
  - parser 对齐 QuickJS `js_parse_property_name` 行为：对象/class 的 computed key 在 `no_in` 外层环境下强制允许 `in`，修复 `accessor-name-computed-in.js` 等解析失败。
  - parser 对象访问器 key 补齐 `IdentifierName/StringLiteral/NumericLiteral/[AssignmentExpression]`，不再仅限标识符。
  - lexer 补齐 `0b/0B` 与 `0o/0O` 数字字面量；清理 object accessor 数字 key 的 parse fail。
  - 子集回归：`language/expressions/object` 从 `259/12` 提升到 `262/9`；`language/expressions/class` 从 `27/20` 提升到 `29/18`。
  - bytecode 新增 `ToPropertyKey` opcode 并用于 object literal computed key/accessor，确保 key coercion 在 value evaluation 前执行。
  - VM 将 `Object.prototype.toString` 与 `Array.prototype.toString` 挂到真实原型对象，移除对象属性读取里的“隐式 toString fallback”。
  - VM `ToPropertyKey` 对对象分支改用 `toString/valueOf` 顺序的最小 `ToPrimitive` 语义；`Object.create(null)` 的 computed key 现可正确抛 TypeError。
  - object 方法/访问器通过 marker 标记为“无 prototype、不可构造”，并修复 `hasOwnProperty('prototype')` 对该标记的判断。
  - VM 补齐 `Object.getOwnPropertyNames`（Object constructor 静态方法），修复 `computed-property-evaluation-order.js` 的 `NotCallable`。
  - 子集回归（latest）：`language/expressions/object` 进一步提升到 `266/5`；`language` 基线进一步提升到 `4735/265`。
  - 对齐 QuickJS `quickjs.c` object literal 分支（`OP_set_proto` 仅在 `PropertyName : AssignmentExpression` 且 key 为 `__proto__` 时触发）：在 AST/bytecode/vm 增加 `ProtoSetter/DefineProtoProperty` 专用链路，shorthand `__proto__` 不再错误改写原型。
  - parser 补齐 array parameter pattern 默认值副作用提取（覆盖 `...[x = expr]`），并对 object/class method 的 `CLASS_METHOD_NO_PROTOTYPE` marker 纳入函数 `length` 预导语句识别，修复 `dflt-params-trailing-comma` 与 `scope-meth-param-rest-elem-var-*`。
  - 子集回归（latest+1）：`language/expressions/object` 提升至 `271/0`；`language` 基线提升至 `4746/254`。
  - parser/bytecode/vm 新增 rest 参数内部 marker（`$__qjs_rest_param__$<index>`）并在调用绑定阶段构造真实 rest 数组，函数 `length` 计算同步纳入 rest 截断规则（取默认参数与 rest 的最早位置）。
  - VM/runtime 补齐 primitive 原型链与方法基线（`String/Number/Boolean` 的 `prototype` 稳定对象、`toString/valueOf`、`String.prototype.charAt/charCodeAt/indexOf/lastIndexOf/split/substring/toLowerCase/toUpperCase`、`Number.prototype.toFixed`），并将 realm 全局属性同步到全局对象（覆盖 `this.parseInt/parseFloat/isNaN/isFinite`）。
  - `Function.prototype` 对齐为可调用值（`typeof Function.prototype === "function"`），并补齐 host function 上的 `toString/valueOf/constructor` 属性。
  - parser 修复相等运算与关系运算优先级（`===/!==` 高于 `in/instanceof/<...`），并在 `Object.prototype` 补齐 `valueOf`，`language/expressions/in` 子集提升至 `15/1`（仅剩 generator `yield` 场景）。
  - VM/runtime 补齐 `Reflect` 最小对象、`Object.defineProperties` 基线路径、`RegExp.prototype.exec` 最小可调用路径，并将 Error 系列构造器改为对象返回；同时 `typeof identifier` 路径补齐全局对象属性回退（含 getter 求值）。
  - parser 修复 conditional `?:` 的 consequent 分支在 `no_in` 上下文中的 `in` 解析（按规范强制 `+In`），`language/expressions/conditional` 子集收敛至 `18/0`。
  - VM `instanceof` 语义补齐：原型链遍历扩展到 function-like 原型值、`Error/TypeError` 原型链稳定化，并支持在 `Object.defineProperty(Function.prototype, "prototype", { get() {} })` 场景下按需触发 getter（primitive LHS 保持不触发）。
  - 子集回归（latest+7）：`language/expressions/property-accessors` 保持 `21/0`，`language/expressions/in` 保持 `15/1`，`language/expressions/typeof` 保持 `13/0`，`language/expressions/conditional` 保持 `18/0`，`language/expressions/instanceof` 提升至 `39/0`，`language/rest-parameters` 保持 `8/0`，`language/expressions/arrow-function` 保持 `71/4`，`language` 基线提升至 `4781/219`。
  - VM `<< >> >>>` 对齐 QuickJS/ECMAScript 左操作数先 `ToInt32/ToUint32` 的求值顺序，并补齐移位 coercion 顺序回归测试。
  - VM `Object.defineProperty` 新增 `HostFunction` 目标支持（含 descriptor/访问器存储、读取、`getOwnPropertyDescriptor` 与 GC 可达性），修复 `function(){}.bind()` 上定义 `prototype` 访问器的 class 继承路径。
  - VM 收紧 `NativeFunction.prototype` 暴露策略（仅构造器路径保留 `prototype`，并保留 `Test262Error` 构造器原型），修复 `class definition/invalid-extends` 语义偏差。
  - parser/VM 为 `class extends` 构造器新增派生标记与 `this` 初始化状态机：未调用 `super()` 前访问 `this` 抛 `ReferenceError`、二次 `super()` 在保持副作用顺序后抛 `ReferenceError`、派生构造器返回值规则收敛（仅允许 object/undefined）。
  - parser/VM 对齐 class constructor `[[Prototype]]` 可见链路：在 `extends` 场景记录构造器父引用并让 `Object.getPrototypeOf(classCtor)`/函数原型读取按该链路回退，修复 `side-effects-in-extends`。
  - VM 将 class constructor 纳入 `caller/arguments` 受限函数集（与 strict/arrow 一致），补齐 `restricted-properties` 断言路径。
  - parser 对 class declaration/expression 注入类名内部词法绑定（`const <ClassName> = $__class_ctor_*`），确保 methods/heritage 捕获独立且不可变的类名引用，修复 `scope-name-lex-*` 失败簇。
  - 子集回归（latest+11）：`language/statements/class/definition` 保持 `33/0`，`language/statements/class` 提升至 `142/46`，`language/expressions/class` 提升至 `32/15`，`language` 基线提升至 `4818/182`。
  - VM 增加“callable prototype”最小语义：`Object.create`/`Object.setPrototypeOf`/`Object.getPrototypeOf` 接受并保留函数值原型（通过 `prototype_value` 存储），用于对齐 `class extends Function` 的原型链行为。
  - derived `super()` 构造路径对齐 QuickJS `new_target` 方向：在 native/host 超类构造返回 object-like 值时，应用派生构造器预分配 `this` 的原型提示；构造返回值判定统一为 object-like（含函数值），并补齐 GC 可达性。
  - class `subclass-builtins` 失败簇收敛：`language/expressions/class/subclass-builtins`=`15/0`，`language/statements/class/subclass-builtins`=`15/0`。
  - 子集回归（latest+12）：`language/statements/class/definition` 保持 `33/0`，`language/statements/class` 提升至 `162/26`，`language/expressions/class` 提升至 `47/0`，`language` 基线提升至 `4856/144`。
  - parser 调整 class lowering 顺序：类名内部绑定注入延后到 `extends` 计算之后，修复 `class x extends x {}` 在 heritage 阶段应抛 `ReferenceError` 的 name-binding 失败簇。
  - parser/VM 为 class heritage function 注入受限标记，并在 arguments 对象上对受限 `callee` 安装 thrower accessor（`TypeError`），修复 `language/statements/class/strict-mode/arguments-callee.js`。
  - 子集回归（latest+13）：`language/statements/class/definition` 保持 `33/0`，`language/statements/class` 提升至 `166/22`，`language/expressions/class` 保持 `47/0`，`language` 基线提升至 `4860/140`。
  - VM 新增 `execute_construct_value` 并将 `BoundCall` 构造路径改为“构造目标函数 + 绑定参数前置（忽略绑定 this）”，修复 class constructor 被 `bind()` 后仍按普通 call 触发 `class constructor cannot be invoked without 'new'` 的偏差。
  - 子集回归（latest+14）：`language/statements/class/definition` 保持 `33/0`，`language/statements/class` 提升至 `168/20`，`language/expressions/class` 保持 `47/0`，`language` 基线提升至 `4862/138`。
  - VM 放宽 derived constructor 返回规则：当显式返回 object-like 值时不再强制要求 `this` 已初始化（`extends null` 显式返回对象语义），修复 `class-definition-null-proto-contains-return-override`。
  - 子集回归（latest+15）：`language/statements/class/definition` 保持 `33/0`，`language/statements/class` 提升至 `169/19`，`language/expressions/class` 保持 `47/0`，`language` 基线提升至 `4863/137`。
  - VM 对齐 QuickJS `set_array_length` 关键路径：数组/数组子类写入更小 `length` 时删除尾部索引属性；并补齐 `Number.prototype.toExponential` 与 `String.prototype.trim`，收敛 `subclass/builtin-objects/{Array(length),Number,String}`。
  - parser/bytecode 为派生构造器拆分 `super()` 与 `super.prop` 基对象：保留 `super` 作为父构造函数调用目标，同时注入独立的 `super.prototype` super-property 基绑定，修复 `language/statements/class/super/in-constructor.js`。
  - 子集回归（latest+16）：`language/statements/class/super` 收敛至 `8/0`，`language/statements/class/subclass` 提升至 `62/15`，`language/statements/class` 提升至 `173/15`，`language` 基线提升至 `4868/132`（快照：`target/test262-language-baseline-5000-20260224-v25.json`）。
  - parser 将 for-of lowering 从“数组快照遍历”升级为“迭代器记录 + `try/finally` 关闭”路径，并在 VM 新增 `Object.__forOfIterator/__forOfStep/__forOfClose` 最小语义；随后通过序列表达式条件保留 completion value，避免 `for-of cptn-*` 回归。
  - 子集回归（latest+17）：`language/statements/class/subclass` 提升至 `64/13`，`language/statements/for-of` 提升至 `63/13`，`language/statements/class` 提升至 `175/13`，`language` 基线提升至 `4871/129`（快照：`target/test262-language-baseline-5000-20260224-v27.json`）。
  - runtime/builtins 新增 `ArrayBuffer/DataView/Map/Set/Promise/Uint8Array` 最小内建构造器与原型路径：接入 `ArrayBuffer.prototype.slice`、`Map.prototype.set`、`Set.prototype.add`、Promise executor 双回调调用、`Uint8Array` index 写入 `ToUint8` 裁剪与 `Object.prototype.toString` 的 `[object Uint8Array]` 标记。
  - 子集回归（latest+18）：`language/statements/class/subclass` 提升至 `75/2`，`language/statements/class` 提升至 `186/2`，`language` 基线提升至 `4882/118`（快照：`target/test262-language-baseline-5000-20260224-v28.json`）。
  - VM Date 构造器补齐最小本地日期分量链路（多参数 `new Date(y, m, d)`）并新增 `Date.prototype.getFullYear/getMonth/getDate/getUTCFullYear/getUTCMonth/getUTCDate` host 路径，收敛 class 内建子类化中的 Date 失败簇。
  - 子集回归（latest+19）：`language/statements/class/subclass` 提升至 `76/1`，`language/statements/class` 提升至 `187/1`，`language` 基线提升至 `4883/117`（快照：`target/test262-language-baseline-5000-20260224-v29.json`）。
  - 对齐 QuickJS `js_function_constructor(..., JS_FUNC_GENERATOR)` 分流：新增 `GeneratorFunctionConstructor` native 路径（构造器拼接逻辑独立于 `Function`），并让 `function*` 闭包 `[[Prototype]]` 走 `GeneratorFunction.prototype`；补齐最小 generator 迭代器 `next()` 返回链路，修复 `class/subclass/builtin-objects/GeneratorFunction/regular-subclassing.js`。
  - 子集回归（latest+20）：`language/statements/class/subclass` 收敛至 `77/0`，`language/statements/class` 收敛至 `188/0`，`language` 基线提升至 `4884/116`（快照：`target/test262-language-baseline-5000-20260224-v30.json`）。
  - VM 为数组迭代补齐 `keys/entries/values/[Symbol.iterator]` 与最小 `Array.prototype.pop`；for-of 数组路径切换为 runtime iterator record，并在 `Object.defineProperty` 数组索引定义时同步 `length`，收敛 `for-of` 的 array contract/error 分支。
  - 子集回归（latest+21）：`language/statements/for-of` 提升至 `74/2`，`language` 基线提升至 `4895/105`（快照：`target/test262-language-baseline-5000-20260224-v32.json`）。
  - parser for-of lowering 将 `finally` 中的 `__forOfClose` 调用改为内部 `let` 声明形态，避免覆盖外层 loop completion；VM 字符串 for-of 改为按 JS code-unit/代理对规则产出迭代值（对齐 astral symbol）。
  - 子集回归（latest+22）：`language/statements/for-of` 收敛至 `76/0`，`language` 基线提升至 `4897/103`（快照：`target/test262-language-baseline-5000-20260224-v33.json`）。
  - parser `catch` 参数新增数组绑定模式降级（临时异常绑定 + let 前置解构声明），修复 `scope-catch-param-*` parse fail，并保持 catch 参数词法环境行为与 QuickJS 方向一致。
  - 子集回归（latest+23）：`language/statements/try` 提升至 `79/12`，`language` 基线提升至 `4899/101`（快照：`target/test262-language-baseline-5000-20260224-v34.json`）。
  - bytecode 对齐 `TryStatement` completion 传播：循环 completion 在每次迭代开始重置，finally 作用域引入“旧 completion 暂存+正常路径恢复”机制，并修正 unwind finally 的 handler pop 顺序（按上下文深度弹栈）。
  - VM 补齐 `Array.prototype.concat` 最小语义与 `Error.prototype.toString`，并将 `ReferenceError/SyntaxError/EvalError/RangeError/URIError` 的实例原型回退到 `Error.prototype`，修复 `try` 目录内的 `NotCallable` 与错误字符串化偏差。
  - 子集回归（latest+24）：`language/statements/try` 收敛至 `91/0`，`language` 基线提升至 `4908/92`（快照：`target/test262-language-baseline-5000-20260224-v35.json`）。

## 3. 分阶段状态

| Phase | 状态 | 证据 | 当前结论 |
| --- | --- | --- | --- |
| Phase 0 | Done | `Cargo.toml`, `docs/quickjs-mapping.md`, `docs/semantics-checklist.md`, `docs/risk-register.md`, `.github/workflows/ci.yml` | 脚手架与基础治理已具备。 |
| Phase 1 | In Progress | `crates/lexer/src/lib.rs`, `crates/parser/src/lib.rs`, `crates/ast/src/lib.rs` | 前端主路径可运行，继续补齐语义边角。 |
| Phase 2 | In Progress | `crates/bytecode/src/lib.rs` | 指令与编译链路已建立，控制流/异常语义持续收敛。 |
| Phase 3 | In Progress | `crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs` | 执行链路可用，仍需进一步完善对象模型与边界语义。 |
| Phase 4 | In Progress | `docs/memory-inventory.md`, `docs/root-strategy.md`, `docs/gc-design.md`, `docs/gc-test-plan.md`, `crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`, `docs/phase4-review.md` | GC 方案、测试计划、PoC、评审与 `ObjectId(slot+generation)` 句柄加固已完成，进入下一轮压力验证与策略细化。 |
| Phase 5 | In Progress | `crates/builtins/src/lib.rs` | 已有 baseline 内建，需继续扩展规范行为。 |
| Phase 6 | Planned | `crates/parser/src/lib.rs`, `crates/vm/src/lib.rs` | ES Module 与微任务队列尚未接通。 |
| Phase 7 | In Progress | `docs/test262-lite.md`, `docs/test262-baseline.md`, `crates/test-harness` | 已有兼容性回归链路，但通过率仍需系统提升。 |

## 4. 当前主要缺口

1. GC 已落地首版 mark-sweep，但仍缺增量/分代策略与更大规模性能压测。
2. `eval/with/strict` 与 descriptor 等复杂语义仍需持续压测与修正。
3. 模块系统与 Promise job queue 尚未启动实现。
4. class 继承链与 regexp 语义仍是 language 子集主失败簇（当前失败集中在 `statements/class/*`、`literals/regexp/*`、`template/*`）；`eval-code/direct` 与 `function-code` 当前子集分别收敛到 `143/0`、`173/0`。

## 5. 下一步执行

- 执行长期任务：`docs/long-horizon-task-phase4.md`（总时长 >8h，含子 agent 并行方案）。
- Phase 4 已完成前六步推进：
  - Step 1: `docs/memory-inventory.md`
  - Step 2: `docs/root-strategy.md`
  - Step 3: `docs/gc-design.md`
  - Step 4: `docs/gc-test-plan.md`
  - Step 5: `crates/vm/src/lib.rs`, `crates/runtime/src/lib.rs`（最小 GC PoC）
  - Step 6: `docs/phase4-review.md`（集成评审与风险收口）
- Phase 4 Step 7 已完成：`crates/vm/src/lib.rs`（`ObjectId slot+generation` 句柄加固 + stale handle 回归）。
- Phase 4 Step 8 已完成（首轮）：新增 8 个 GC 压力样例并完成 19/19 回归，为 Step 10 规模化扩展建立基线。
- Phase 4 Step 9 完成：Default/Stress profile 触发/观测校验闭环，命令与快照都符合预期。
  - Default Profile command: `test262-run --show-gc`（默认 auto/runtime 关闭）with VM regression asserting `gc_stats == GcStats::default()` (zeroed counters) and `boundary_collections == collections_total` while `runtime_collections == 0`; latest snapshot `collections_total=0`, `boundary_collections=0`, `runtime_collections=0`.
  - Stress Profile command: `test262-lite --auto-gc --runtime-gc --auto-gc-threshold 1 --runtime-gc-interval 1` plus `test262-run --show-gc` snapshot showing `collections_total=29283`, `boundary_collections=22`, `runtime_collections=29261`, `reclaimed_objects=611` and confirming `collections_total == runtime_collections + boundary_collections`.
- Phase 4 Step 10 已启动：GC 压测样例已扩展至 26 个总样例（含 18 个 `gc-*`），并新增快照报告 `docs/gc-snapshot-report.md`。
- 自动 GC 已支持开关+阈值（执行边界触发，默认关闭）。
- `test262-lite` 已接入 `gc-*` 样例（闭包捕获、异常 unwind、with、闭包链、循环引用、循环闭包）并在集成测试中启用自动 GC 压测模式。
- `test262-run` CLI 已支持 `--auto-gc` / `--auto-gc-threshold`。
- `test262-run` CLI 已支持 `--runtime-gc` / `--runtime-gc-interval`（安全点模式）。
- `test262-run` CLI 已支持 GC guard 阈值参数与基线文件模式：`--expect-gc-baseline` + `--expect-*`（显式参数优先），可作为 CI 回归门槛。
- VM 已支持运行中安全点 GC（`enable_runtime_gc` + `set_runtime_gc_check_interval`）。
- `gc_stats` 已提供对象规模与 mark/sweep 耗时观测字段。
- 已接入 `HostPinRoot` 最小 API（pin/unpin）并有回归测试覆盖。
- VM 已接入 `ObjectId(slot+generation)`，并新增 stale handle 回归测试确保回收复用安全。
- `test262-lite` 在 `--auto-gc --runtime-gc` 模式下当前 26/26 通过。
- `test262-run --show-gc` 已可输出套件级 GC 聚合统计；最新 stress 快照：`collections_total=29283`、`boundary_collections=22`、`runtime_collections=29261`、`reclaimed_objects=611`。
- `crates/test-harness/tests/test262_lite.rs` 已增加 GC 守护断言：`reclaimed_objects > 0` 且 runtime ratio `>= 0.9`。
- `array churn + runtime GC` 的 `UnknownObject` 问题已通过 `gc_shadow_roots` 修复并加入 VM 回归测试，进入持续监控阶段。
- 以 test262 失败簇驱动 builtins 与语义缺口收敛，持续更新基线文档。
