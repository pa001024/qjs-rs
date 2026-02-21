#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Number(f64),
    Bool(bool),
    Function(u64),
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
