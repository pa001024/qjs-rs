#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, NativeFunction, Realm};
use vm::Vm;

#[test]
fn core_builtins_object_array_boolean_function() {
    let script = parse_script(
        "var ok = true;\
         ok = ok && Object.prototype.toString.call({}) === '[object Object]';\
         var arr = [1, 2, 3];\
         arr.length = 1;\
         ok = ok && arr.length === 1 && arr[1] === undefined;\
         ok = ok && Boolean(false) === false && (new Boolean(false)).valueOf() === false;\
         ok = ok && Function.length === 1 && Function.prototype.constructor === Function;\
         var add = Function('a', 'b', 'return a + b;');\
         ok = ok && add(20, 22) === 42;\
         var ctor = new Function('return this === globalThis;');\
         ok = ok && ctor() === true;\
         var coercionThrow = false;\
         try { Function({ toString: function() { throw 7; } }); } catch (err) { coercionThrow = err === 7; }\
         ok = ok && coercionThrow;\
         var syntaxErr = false;\
         try { Function('['); } catch (err) { syntaxErr = err instanceof SyntaxError; }\
         ok && syntaxErr;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);

    let mut realm = Realm::default();
    realm.define_global(
        "Object",
        JsValue::NativeFunction(NativeFunction::ObjectConstructor),
    );
    realm.define_global(
        "Array",
        JsValue::NativeFunction(NativeFunction::ArrayConstructor),
    );
    realm.define_global(
        "Boolean",
        JsValue::NativeFunction(NativeFunction::BooleanConstructor),
    );
    realm.define_global(
        "Function",
        JsValue::NativeFunction(NativeFunction::FunctionConstructor),
    );
    realm.define_global(
        "SyntaxError",
        JsValue::NativeFunction(NativeFunction::SyntaxErrorConstructor),
    );

    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}

#[test]
fn core_builtins_string_number_math() {
    let script = parse_script(
        "var ok = true;\
         ok = ok && String('abc') === 'abc';\
         ok = ok && String.fromCharCode(65, 66, 67) === 'ABC';\
         ok = ok && Number('42') === 42;\
         ok = ok && Number.isFinite(1) && !Number.isFinite('1');\
         ok = ok && Number.isInteger(-0) && Number.isSafeInteger(9007199254740991) && !Number.isSafeInteger(9007199254740992);\
         ok = ok && Number.isNaN(Number('x'));\
         ok = ok && Math.sign(-0) === -0 && (1 / Math.sign(-0)) === -Infinity;\
         ok = ok && Math.clz32(1) === 31;\
         ok = ok && Math.hypot(3, 4) === 5;\
         ok = ok && Math.log2(8) === 3 && Math.log10(1000) === 3;\
         ok = ok && Math.acosh(1) === 0;\
         var threw = false;\
         try { String.fromCharCode({ valueOf: function() { throw 'boom'; } }); } catch (err) { threw = err === 'boom'; }\
         ok && threw;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);

    let mut realm = Realm::default();
    realm.define_global(
        "String",
        JsValue::NativeFunction(NativeFunction::StringConstructor),
    );
    realm.define_global(
        "Number",
        JsValue::NativeFunction(NativeFunction::NumberConstructor),
    );
    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}

#[test]
fn promise_static_surface_baseline() {
    let script = parse_script(
        "typeof Promise.resolve === 'function' \
         && typeof Promise.reject === 'function' \
         && typeof Promise.all === 'function' \
         && typeof Promise.any === 'function' \
         && typeof Promise.race === 'function' \
         && typeof Promise.allSettled === 'function';",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);

    let mut realm = Realm::default();
    realm.define_global(
        "Promise",
        JsValue::NativeFunction(NativeFunction::PromiseConstructor),
    );

    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}
