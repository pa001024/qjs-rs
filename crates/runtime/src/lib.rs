#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFunction {
    Eval,
    FunctionConstructor,
    ObjectConstructor,
    ArrayConstructor,
    ObjectKeys,
    ObjectCreate,
    ObjectSetPrototypeOf,
    ObjectDefineProperty,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetPrototypeOf,
    ObjectIsExtensible,
    NumberConstructor,
    BooleanConstructor,
    DateConstructor,
    RegExpConstructor,
    StringConstructor,
    StringFromCharCode,
    SymbolConstructor,
    IsNaN,
    Assert,
    Test262Error,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Number(f64),
    Bool(bool),
    Null,
    String(String),
    Function(u64),
    NativeFunction(NativeFunction),
    HostFunction(u64),
    Object(u64),
    Undefined,
}

#[derive(Debug, Default)]
pub struct Realm {
    globals: BTreeMap<String, JsValue>,
}

impl Realm {
    pub fn define_global(&mut self, name: &str, value: JsValue) {
        self.globals.insert(name.to_string(), value);
    }

    pub fn get_global(&self, name: &str) -> Option<&JsValue> {
        self.globals.get(name)
    }

    pub fn resolve_identifier(&self, name: &str) -> Option<JsValue> {
        self.globals.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::{JsValue, Realm};

    #[test]
    fn resolves_identifier_from_globals() {
        let mut realm = Realm::default();
        realm.define_global("answer", JsValue::Number(42.0));
        assert_eq!(
            realm.resolve_identifier("answer"),
            Some(JsValue::Number(42.0))
        );
        assert_eq!(realm.resolve_identifier("missing"), None);
    }
}
