#![forbid(unsafe_code)]

use runtime::{JsValue, Realm};

pub fn install_baseline(realm: &mut Realm) {
    realm.define_global("NaN", JsValue::Number(f64::NAN));
    realm.define_global("Infinity", JsValue::Number(f64::INFINITY));
}
