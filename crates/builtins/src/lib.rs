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
}
