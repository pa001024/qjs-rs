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
