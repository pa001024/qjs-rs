#![forbid(unsafe_code)]

use runtime::{JsValue, NativeFunction, Realm};

pub fn install_baseline(realm: &mut Realm) {
    realm.define_global("NaN", JsValue::Number(f64::NAN));
    realm.define_global("Infinity", JsValue::Number(f64::INFINITY));
    realm.define_global("eval", JsValue::NativeFunction(NativeFunction::Eval));
    realm.define_global(
        "Function",
        JsValue::NativeFunction(NativeFunction::FunctionConstructor),
    );
    realm.define_global(
        "Object",
        JsValue::NativeFunction(NativeFunction::ObjectConstructor),
    );
    realm.define_global(
        "Number",
        JsValue::NativeFunction(NativeFunction::NumberConstructor),
    );
    realm.define_global(
        "String",
        JsValue::NativeFunction(NativeFunction::StringConstructor),
    );
    realm.define_global(
        "RegExp",
        JsValue::NativeFunction(NativeFunction::RegExpConstructor),
    );
    realm.define_global(
        "Symbol",
        JsValue::NativeFunction(NativeFunction::SymbolConstructor),
    );
    realm.define_global("isNaN", JsValue::NativeFunction(NativeFunction::IsNaN));
    realm.define_global("assert", JsValue::NativeFunction(NativeFunction::Assert));
    realm.define_global(
        "Test262Error",
        JsValue::NativeFunction(NativeFunction::Test262Error),
    );
    realm.define_global(
        "Error",
        JsValue::NativeFunction(NativeFunction::Test262Error),
    );
    realm.define_global(
        "TypeError",
        JsValue::NativeFunction(NativeFunction::Test262Error),
    );
    realm.define_global(
        "ReferenceError",
        JsValue::NativeFunction(NativeFunction::Test262Error),
    );
    realm.define_global(
        "SyntaxError",
        JsValue::NativeFunction(NativeFunction::Test262Error),
    );
}
