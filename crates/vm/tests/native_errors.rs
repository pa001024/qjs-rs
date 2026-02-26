#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, NativeFunction, Realm};
use vm::Vm;

#[test]
fn native_error_constructor_prototype_chain() {
    let script = parse_script(
        "var ctors = [TypeError, ReferenceError, SyntaxError, RangeError, EvalError, URIError];\
         var ok = true;\
         for (var i = 0; i < ctors.length; i++) {\
           var C = ctors[i];\
           ok = ok && C.prototype !== Error.prototype;\
           ok = ok && C.prototype.constructor === C;\
           ok = ok && Object.getPrototypeOf(C.prototype) === Error.prototype;\
           var e = new C('boom');\
           ok = ok && Object.getPrototypeOf(e) === C.prototype;\
           ok = ok && (e instanceof C) && (e instanceof Error);\
         }\
         ok;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    realm.define_global(
        "Object",
        JsValue::NativeFunction(NativeFunction::ObjectConstructor),
    );
    realm.define_global(
        "Error",
        JsValue::NativeFunction(NativeFunction::ErrorConstructor),
    );
    realm.define_global(
        "TypeError",
        JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
    );
    realm.define_global(
        "ReferenceError",
        JsValue::NativeFunction(NativeFunction::ReferenceErrorConstructor),
    );
    realm.define_global(
        "SyntaxError",
        JsValue::NativeFunction(NativeFunction::SyntaxErrorConstructor),
    );
    realm.define_global(
        "RangeError",
        JsValue::NativeFunction(NativeFunction::RangeErrorConstructor),
    );
    realm.define_global(
        "EvalError",
        JsValue::NativeFunction(NativeFunction::EvalErrorConstructor),
    );
    realm.define_global(
        "URIError",
        JsValue::NativeFunction(NativeFunction::URIErrorConstructor),
    );
    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}
