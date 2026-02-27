#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeFunction {
    Eval,
    FunctionConstructor,
    GeneratorFunctionConstructor,
    ObjectConstructor,
    ArrayConstructor,
    ArrayIsArray,
    ObjectKeys,
    ObjectEntries,
    ObjectValues,
    ObjectGetOwnPropertyNames,
    ObjectCreate,
    ObjectAssign,
    ObjectSetPrototypeOf,
    ObjectDefineProperty,
    ObjectDefineProperties,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetOwnPropertyDescriptors,
    ObjectGetPrototypeOf,
    ObjectPreventExtensions,
    ObjectIsExtensible,
    ObjectIsSealed,
    ObjectIsFrozen,
    ObjectIs,
    ObjectFreeze,
    ObjectSeal,
    ObjectForInKeys,
    ObjectForOfValues,
    ObjectForOfIterator,
    ObjectForOfStep,
    ObjectForOfClose,
    ObjectGetTemplateObject,
    ObjectTdzMarker,
    NumberConstructor,
    NumberIsFinite,
    NumberIsInteger,
    NumberIsSafeInteger,
    BooleanConstructor,
    ArrayBufferConstructor,
    DataViewConstructor,
    MapConstructor,
    SetConstructor,
    WeakMapConstructor,
    WeakSetConstructor,
    ProxyConstructor,
    PromiseConstructor,
    PromiseThen,
    PromiseCatch,
    PromiseFinally,
    Uint8ArrayConstructor,
    DateConstructor,
    DateParse,
    DateUtc,
    DatePrototypeMethod,
    RegExpConstructor,
    MathAbs,
    MathAcos,
    MathAcosh,
    MathAsin,
    MathAsinh,
    MathAtan,
    MathAtanh,
    MathAtan2,
    MathCbrt,
    MathCeil,
    MathClz32,
    MathCos,
    MathCosh,
    MathExp,
    MathExpm1,
    MathFloor,
    MathFround,
    MathHypot,
    MathImul,
    MathLog,
    MathLog1p,
    MathLog10,
    MathLog2,
    MathMax,
    MathMin,
    MathPow,
    MathRandom,
    MathRound,
    MathSign,
    MathSinh,
    MathTrunc,
    MathSin,
    MathSqrt,
    MathTan,
    MathTanh,
    StringConstructor,
    StringFromCharCode,
    SymbolConstructor,
    IsNaN,
    IsFinite,
    ParseInt,
    ParseFloat,
    DecodeURI,
    DecodeURIComponent,
    EncodeURI,
    EncodeURIComponent,
    Escape,
    Unescape,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleLifecycleState {
    Unlinked,
    Linking,
    Linked,
    Evaluating,
    Evaluated,
    Errored,
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
