#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Number(f64),
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
}
