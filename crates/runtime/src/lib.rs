#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFunction {
    Eval,
    FunctionConstructor,
    GeneratorFunctionConstructor,
    ObjectConstructor,
    ArrayConstructor,
    ArrayIsArray,
    ObjectKeys,
    ObjectGetOwnPropertyNames,
    ObjectCreate,
    ObjectSetPrototypeOf,
    ObjectDefineProperty,
    ObjectDefineProperties,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetPrototypeOf,
    ObjectIsExtensible,
    ObjectFreeze,
    ObjectForInKeys,
    ObjectForOfValues,
    ObjectForOfIterator,
    ObjectForOfStep,
    ObjectForOfClose,
    ObjectTdzMarker,
    NumberConstructor,
    BooleanConstructor,
    ArrayBufferConstructor,
    DataViewConstructor,
    MapConstructor,
    SetConstructor,
    PromiseConstructor,
    Uint8ArrayConstructor,
    DateConstructor,
    DateParse,
    DateUtc,
    DatePrototypeMethod,
    RegExpConstructor,
    MathAbs,
    MathAcos,
    MathAsin,
    MathAtan,
    MathAtan2,
    MathCeil,
    MathCos,
    MathExp,
    MathFloor,
    MathLog,
    MathMax,
    MathMin,
    MathPow,
    MathRandom,
    MathRound,
    MathSin,
    MathSqrt,
    MathTan,
    StringConstructor,
    StringFromCharCode,
    SymbolConstructor,
    IsNaN,
    IsFinite,
    ParseInt,
    ParseFloat,
    Assert,
    Test262Error,
    ErrorConstructor,
    TypeErrorConstructor,
    ReferenceErrorConstructor,
    SyntaxErrorConstructor,
    EvalErrorConstructor,
    RangeErrorConstructor,
    URIErrorConstructor,
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
    Uninitialized,
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

    pub fn globals_values(&self) -> impl Iterator<Item = &JsValue> {
        self.globals.values()
    }

    pub fn globals_entries(&self) -> impl Iterator<Item = (&str, &JsValue)> {
        self.globals
            .iter()
            .map(|(name, value)| (name.as_str(), value))
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
