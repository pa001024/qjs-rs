#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::run_script;

fn assert_script_bool(source: &str) {
    let result = run_script(source, &[]);
    assert_eq!(
        result,
        Ok(JsValue::Bool(true)),
        "unexpected result for script:\n{source}"
    );
}

#[test]
fn json_parse_supports_nested_reviver_transforms() {
    assert_script_bool(
        "var source = '{\"keep\":1,\"drop\":2,\"nested\":{\"value\":3},\"arr\":[1,2,3]}'; \
         var parsed = JSON.parse(source, function(key, value) { \
           if (key === 'drop') { return undefined; } \
           if (key === 'value') { return value * 10; } \
           if (key === '1') { return value + 40; } \
           return value; \
         }); \
         parsed.keep === 1 && !('drop' in parsed) && parsed.nested.value === 30 && \
         parsed.arr[1] === 42 && parsed.arr.length === 3;",
    );
}

#[test]
fn json_parse_malformed_input_throws_syntax_error() {
    assert_script_bool(
        "var threw = false; \
         try { JSON.parse('{\"x\":'); } catch (err) { threw = err instanceof SyntaxError; } \
         threw;",
    );
}

#[test]
fn json_stringify_supports_replacer_space_and_value_filtering() {
    assert_script_bool(
        "var input = { keep: 1, drop: 2, nested: { value: 3 }, arr: [1, undefined, function() {}, 4] }; \
         var pretty = JSON.stringify(input, function(key, value) { \
           if (key === 'drop') { return undefined; } \
           if (key === 'value') { return value + 7; } \
           return value; \
         }, 2); \
         var list = JSON.stringify({ a: 1, b: 2, c: 3 }, ['c', 'a'], '..........++++'); \
         var arrayFiltered = JSON.stringify([1, undefined, function() {}, 4]); \
         pretty.indexOf('\\n  \"nested\": {\\n    \"value\": 10\\n  }') !== -1 && \
         pretty.indexOf('\"drop\"') === -1 && \
         pretty.indexOf('\"arr\": [\\n    1,\\n    null,\\n    null,\\n    4\\n  ]') !== -1 && \
         list === '{\\n..........\"c\": 3,\\n..........\"a\": 1\\n}' && \
         arrayFiltered === '[1,null,null,4]';",
    );
}

#[test]
fn json_stringify_cyclic_values_throw_type_error() {
    assert_script_bool(
        "var cycle = {}; cycle.self = cycle; \
         var threw = false; \
         try { JSON.stringify(cycle); } catch (err) { threw = err instanceof TypeError; } \
         threw;",
    );
}
