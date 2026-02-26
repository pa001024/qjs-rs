#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, NativeFunction, Realm};
use vm::Vm;

#[test]
fn json_parse_reviver_semantics() {
    let script = parse_script(
        "var source = '{\"keep\":1,\"drop\":2,\"nested\":{\"value\":3},\"arr\":[1,2,3]}';\
         var parsed = JSON.parse(source, function(key, value) {\
           if (key === 'drop') { return undefined; }\
           if (key === 'value') { return value * 10; }\
           if (key === '1') { return value + 40; }\
           return value;\
         });\
         var malformed = false;\
         try { JSON.parse('{\"x\":'); } catch (err) { malformed = err instanceof SyntaxError; }\
         parsed.keep === 1 && !('drop' in parsed) && parsed.nested.value === 30 &&\
         parsed.arr[1] === 42 && parsed.arr.length === 3 && malformed;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    realm.define_global(
        "SyntaxError",
        JsValue::NativeFunction(NativeFunction::SyntaxErrorConstructor),
    );
    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}
