#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, NativeFunction, Realm};
use vm::Vm;

#[test]
fn native_error_constructor_prototype_chain() {
    let script = parse_script(
        "var ctors = [TypeError, ReferenceError, SyntaxError, RangeError, EvalError, URIError, AggregateError];\
         var ok = true;\
         for (var i = 0; i < ctors.length; i++) {\
           var C = ctors[i];\
           ok = ok && C.prototype !== Error.prototype;\
           ok = ok && C.prototype.constructor === C;\
           ok = ok && Object.getPrototypeOf(C.prototype) === Error.prototype;\
           var e = (C === AggregateError) ? new C([], 'boom') : new C('boom');\
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
    realm.define_global(
        "AggregateError",
        JsValue::NativeFunction(NativeFunction::AggregateErrorConstructor),
    );
    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}

#[test]
fn host_callback_set_prototype_of_reports_type_errors_for_invalid_mutations() {
    let script = parse_script(
        "var base = {}; \
         Object.setPrototypeOf(HostCtor, base); \
         var cycle = false; \
         try { Object.setPrototypeOf(base, HostCtor); } catch (err) { cycle = err instanceof TypeError; } \
         Object.preventExtensions(HostCtor); \
         var blocked = false; \
         try { Object.setPrototypeOf(HostCtor, {}); } catch (err) { blocked = err instanceof TypeError; } \
         cycle && blocked;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);

    let mut realm = Realm::default();
    realm.define_global(
        "Object",
        JsValue::NativeFunction(NativeFunction::ObjectConstructor),
    );
    realm.define_global(
        "TypeError",
        JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
    );

    let mut vm = Vm::default();
    let host_ctor = vm.register_host_callback_function(
        "HostCtor",
        0.0,
        true,
        |_vm, this_arg, _args, _realm, _strict| Ok(this_arg.unwrap_or(JsValue::Undefined)),
    );
    realm.define_global("HostCtor", host_ctor);

    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}
