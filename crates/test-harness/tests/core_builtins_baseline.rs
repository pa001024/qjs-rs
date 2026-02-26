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

#[test]
fn string_number_math_baseline() {
    assert_script(
        "var ok = true; \
         ok = ok && String('abc') === 'abc'; \
         ok = ok && String.fromCharCode(65, 66, 67) === 'ABC'; \
         ok = ok && Number('42') === 42; \
         ok = ok && Number.isFinite(1) && !Number.isFinite('1'); \
         ok = ok && Number.isInteger(-0) && Number.isSafeInteger(9007199254740991) && !Number.isSafeInteger(9007199254740992); \
         ok = ok && Number.isNaN(Number('x')); \
         ok = ok && Math.sign(-0) === -0 && (1 / Math.sign(-0)) === -Infinity; \
         ok = ok && Math.clz32(1) === 31; \
         ok = ok && Math.hypot(3, 4) === 5; \
         ok = ok && Math.log2(8) === 3 && Math.log10(1000) === 3; \
         ok = ok && Math.acosh(1) === 0; \
         var threw = false; \
         try { String.fromCharCode({ valueOf: function() { throw 'boom'; } }); } catch (err) { threw = err === 'boom'; } \
         ok && threw;",
        JsValue::Bool(true),
    );
}
