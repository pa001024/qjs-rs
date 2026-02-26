#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, NativeFunction, Realm};
use vm::Vm;

#[test]
fn json_stringify_replacer_space_cycle() {
    let script = parse_script(
        "var input = { keep: 1, drop: 2, nested: { value: 3 }, arr: [1, undefined, function() {}, 4] };\
         var pretty = JSON.stringify(input, function(key, value) {\
           if (key === 'drop') { return undefined; }\
           if (key === 'value') { return value + 7; }\
           return value;\
         }, 2);\
         var list = JSON.stringify({ a: 1, b: 2, c: 3 }, ['c', 'a'], '..........++++');\
         var arrayFiltered = JSON.stringify([1, undefined, function() {}, 4]);\
         var cyclic = {}; cyclic.self = cyclic;\
         var cycleErr = false;\
         try { JSON.stringify(cyclic); } catch (err) { cycleErr = err instanceof TypeError; }\
         pretty.indexOf('\\n  \"nested\": {\\n    \"value\": 10\\n  }') !== -1 &&\
         pretty.indexOf('\"drop\"') === -1 &&\
         pretty.indexOf('\"arr\": [\\n    1,\\n    null,\\n    null,\\n    4\\n  ]') !== -1 &&\
         list === '{\\n..........\"c\": 3,\\n..........\"a\": 1\\n}' &&\
         arrayFiltered === '[1,null,null,4]' && cycleErr;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    realm.define_global(
        "TypeError",
        JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
    );
    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}
