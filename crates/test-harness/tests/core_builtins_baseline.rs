#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::run_script;

fn assert_script(source: &str, expected: JsValue) {
    assert_eq!(run_script(source, &[]), Ok(expected));
}

#[test]
fn object_array_boolean_function_baseline() {
    assert_script(
        "var ok = true; \
         ok = ok && Object.prototype.toString.call({}) === '[object Object]'; \
         var arr = [1, 2, 3]; arr.length = 1; \
         ok = ok && arr.length === 1 && arr[1] === undefined; \
         ok = ok && Boolean(false) === false && (new Boolean(false)).valueOf() === false; \
         ok = ok && Function.length === 1 && Function.prototype.constructor === Function; \
         var add = Function('a', 'b', 'return a + b;'); \
         ok = ok && add(20, 22) === 42; \
         var coercionThrow = false; \
         try { Function({ toString: function() { throw 7; } }); } catch (err) { coercionThrow = err === 7; } \
         ok = ok && coercionThrow; \
         var syntaxErr = false; \
         try { Function('['); } catch (err) { syntaxErr = err instanceof SyntaxError; } \
         ok && syntaxErr;",
        JsValue::Bool(true),
    );
}
