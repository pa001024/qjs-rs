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
    ObjectGetOwnPropertySymbols,
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
    ObjectForAwaitIterator,
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
    PromiseResolve,
    PromiseReject,
    PromiseAll,
    PromiseAny,
    PromiseRace,
    PromiseAllSettled,
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
    IsHTMLDDA,
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
    AggregateErrorConstructor,
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

impl From<bool> for JsValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f64> for JsValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<f32> for JsValue {
    fn from(value: f32) -> Self {
        Self::Number(value as f64)
    }
}

impl From<i32> for JsValue {
    fn from(value: i32) -> Self {
        Self::Number(value as f64)
    }
}

impl From<i64> for JsValue {
    fn from(value: i64) -> Self {
        Self::Number(value as f64)
    }
}

impl From<u32> for JsValue {
    fn from(value: u32) -> Self {
        Self::Number(value as f64)
    }
}

impl From<u64> for JsValue {
    fn from(value: u64) -> Self {
        Self::Number(value as f64)
    }
}

impl From<usize> for JsValue {
    fn from(value: usize) -> Self {
        Self::Number(value as f64)
    }
}

impl From<String> for JsValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for JsValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl<T> From<Option<T>> for JsValue
where
    T: Into<JsValue>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Self::Null,
        }
    }
}

#[macro_export]
macro_rules! js_value {
    (null) => {
        $crate::JsValue::Null
    };
    (undefined) => {
        $crate::JsValue::Undefined
    };
    (uninitialized) => {
        $crate::JsValue::Uninitialized
    };
    (true) => {
        $crate::JsValue::Bool(true)
    };
    (false) => {
        $crate::JsValue::Bool(false)
    };
    ([$($value:tt),* $(,)?]) => {{
        let mut values: ::std::vec::Vec<$crate::JsValue> = ::std::vec::Vec::new();
        $(
            let value: $crate::JsValue = $crate::js_value!($value);
            values.push(value);
        )*
        values
    }};
    ($value:expr) => {{
        let value: $crate::JsValue = ::core::convert::Into::into($value);
        value
    }};
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

#[derive(Debug, Default, Clone)]
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

    #[test]
    fn js_value_macro_converts_common_literals() {
        assert_eq!(crate::js_value!(true), JsValue::Bool(true));
        assert_eq!(crate::js_value!(false), JsValue::Bool(false));
        assert_eq!(crate::js_value!(null), JsValue::Null);
        assert_eq!(crate::js_value!(undefined), JsValue::Undefined);
        assert_eq!(crate::js_value!(41), JsValue::Number(41.0));
        assert_eq!(
            crate::js_value!("hello"),
            JsValue::String("hello".to_string())
        );
    }

    #[test]
    fn js_value_macro_converts_array_literals() {
        let value = crate::js_value!([1, true, null, "hello", undefined,]);
        assert_eq!(
            value,
            vec![
                JsValue::Number(1.0),
                JsValue::Bool(true),
                JsValue::Null,
                JsValue::String("hello".to_string()),
                JsValue::Undefined,
            ]
        );
    }
}
