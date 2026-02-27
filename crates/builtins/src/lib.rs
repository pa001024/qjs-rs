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
        "Array",
        JsValue::NativeFunction(NativeFunction::ArrayConstructor),
    );
    realm.define_global(
        "Number",
        JsValue::NativeFunction(NativeFunction::NumberConstructor),
    );
    realm.define_global(
        "Boolean",
        JsValue::NativeFunction(NativeFunction::BooleanConstructor),
    );
    realm.define_global(
        "ArrayBuffer",
        JsValue::NativeFunction(NativeFunction::ArrayBufferConstructor),
    );
    realm.define_global(
        "DataView",
        JsValue::NativeFunction(NativeFunction::DataViewConstructor),
    );
    realm.define_global(
        "Map",
        JsValue::NativeFunction(NativeFunction::MapConstructor),
    );
    realm.define_global(
        "Set",
        JsValue::NativeFunction(NativeFunction::SetConstructor),
    );
    realm.define_global(
        "Promise",
        JsValue::NativeFunction(NativeFunction::PromiseConstructor),
    );
    realm.define_global(
        "Proxy",
        JsValue::NativeFunction(NativeFunction::ProxyConstructor),
    );
    realm.define_global(
        "Uint8Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    // Minimal typed array surface: additional constructors are currently aliases
    // to the existing Uint8Array baseline constructor.
    realm.define_global(
        "Int8Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "Uint8ClampedArray",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "Int16Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "Uint16Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "Int32Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "Uint32Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "Float32Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "Float64Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "BigInt64Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "BigUint64Array",
        JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
    );
    realm.define_global(
        "WeakMap",
        JsValue::NativeFunction(NativeFunction::WeakMapConstructor),
    );
    realm.define_global(
        "WeakSet",
        JsValue::NativeFunction(NativeFunction::WeakSetConstructor),
    );
    realm.define_global(
        "Date",
        JsValue::NativeFunction(NativeFunction::DateConstructor),
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
    realm.define_global(
        "isFinite",
        JsValue::NativeFunction(NativeFunction::IsFinite),
    );
    realm.define_global(
        "parseInt",
        JsValue::NativeFunction(NativeFunction::ParseInt),
    );
    realm.define_global(
        "parseFloat",
        JsValue::NativeFunction(NativeFunction::ParseFloat),
    );
    realm.define_global(
        "decodeURI",
        JsValue::NativeFunction(NativeFunction::DecodeURI),
    );
    realm.define_global(
        "decodeURIComponent",
        JsValue::NativeFunction(NativeFunction::DecodeURIComponent),
    );
    realm.define_global(
        "encodeURI",
        JsValue::NativeFunction(NativeFunction::EncodeURI),
    );
    realm.define_global(
        "encodeURIComponent",
        JsValue::NativeFunction(NativeFunction::EncodeURIComponent),
    );
    realm.define_global("escape", JsValue::NativeFunction(NativeFunction::Escape));
    realm.define_global(
        "unescape",
        JsValue::NativeFunction(NativeFunction::Unescape),
    );
    realm.define_global("assert", JsValue::NativeFunction(NativeFunction::Assert));
    realm.define_global(
        "Test262Error",
        JsValue::NativeFunction(NativeFunction::Test262Error),
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
        "EvalError",
        JsValue::NativeFunction(NativeFunction::EvalErrorConstructor),
    );
    realm.define_global(
        "RangeError",
        JsValue::NativeFunction(NativeFunction::RangeErrorConstructor),
    );
    realm.define_global(
        "URIError",
        JsValue::NativeFunction(NativeFunction::URIErrorConstructor),
    );
}

#[cfg(test)]
mod tests {
    use super::install_baseline;
    use runtime::{JsValue, NativeFunction, Realm};

    #[test]
    fn installs_parse_int_global() {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        assert_eq!(
            realm.get_global("parseInt"),
            Some(&JsValue::NativeFunction(NativeFunction::ParseInt))
        );
    }

    #[test]
    fn installs_parse_float_global() {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        assert_eq!(
            realm.get_global("parseFloat"),
            Some(&JsValue::NativeFunction(NativeFunction::ParseFloat))
        );
    }

    #[test]
    fn installs_is_finite_global() {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        assert_eq!(
            realm.get_global("isFinite"),
            Some(&JsValue::NativeFunction(NativeFunction::IsFinite))
        );
    }

    #[test]
    fn installs_escape_global() {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        assert_eq!(
            realm.get_global("escape"),
            Some(&JsValue::NativeFunction(NativeFunction::Escape))
        );
    }

    #[test]
    fn installs_unescape_global() {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        assert_eq!(
            realm.get_global("unescape"),
            Some(&JsValue::NativeFunction(NativeFunction::Unescape))
        );
    }

    #[test]
    fn installs_additional_error_globals() {
        let mut realm = Realm::default();
        install_baseline(&mut realm);
        assert_eq!(
            realm.get_global("EvalError"),
            Some(&JsValue::NativeFunction(
                NativeFunction::EvalErrorConstructor
            ))
        );
        assert_eq!(
            realm.get_global("RangeError"),
            Some(&JsValue::NativeFunction(
                NativeFunction::RangeErrorConstructor
            ))
        );
        assert_eq!(
            realm.get_global("URIError"),
            Some(&JsValue::NativeFunction(
                NativeFunction::URIErrorConstructor
            ))
        );
    }
}
