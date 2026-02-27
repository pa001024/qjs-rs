#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::run_script;

fn assert_script(source: &str, expected: JsValue) {
    assert_eq!(run_script(source, &[]), Ok(expected));
}

#[test]
fn regexp_constructor_clone_and_flag_preservation() {
    assert_script(
        "var ok = true; \
         var original = /foo/gi; \
         var clone = new RegExp(original); \
         ok = ok && clone !== original && clone.toString() === '/foo/gi'; \
         ok = ok && clone.global && clone.ignoreCase && !clone.multiline && !clone.sticky; \
         var overridden = new RegExp(original, 'ym'); \
         ok = ok && overridden.toString() === '/foo/my' && overridden.multiline && overridden.sticky; \
         ok = ok && !overridden.global && !overridden.ignoreCase; \
         var allFlags = new RegExp('x', 'ygmius'); \
         ok && allFlags.toString() === '/x/gimsuy';",
        JsValue::Bool(true),
    );
}

#[test]
fn regexp_exec_and_test_share_last_index_contract() {
    assert_script(
        "var ok = true; \
         var global = /a/g; \
         global.lastIndex = 1; \
         ok = ok && global.exec('ba').index === 1 && global.lastIndex === 2; \
         ok = ok && global.exec('ba') === null && global.lastIndex === 0; \
         global.lastIndex = 1; \
         ok = ok && global.test('ba') === true && global.lastIndex === 2; \
         ok = ok && global.test('ba') === false && global.lastIndex === 0; \
         var sticky = /a/y; \
         sticky.lastIndex = 1; \
         ok = ok && sticky.test('ba') === true && sticky.lastIndex === 2; \
         sticky.lastIndex = 0; \
         ok = ok && sticky.exec('ba') === null && sticky.lastIndex === 0; \
         var plain = /a/; \
         plain.lastIndex = 1; \
         ok = ok && plain.test('ba') === true && plain.lastIndex === 1; \
         var frozen = /a/g; \
         Object.defineProperty(frozen, 'lastIndex', { value: 0, writable: false }); \
         var threw = false; \
         try { frozen.test('a'); } catch (e) { threw = e instanceof TypeError; } \
         ok && threw && frozen.lastIndex === 0;",
        JsValue::Bool(true),
    );
}

#[test]
fn regexp_exec_materializes_capture_slots_index_and_input() {
    assert_script(
        "var ok = true; \
         var rx = /(a)(b)?/g; \
         var first = rx.exec('ab a'); \
         ok = ok && first !== null && first[0] === 'ab' && first[1] === 'a' && first[2] === 'b'; \
         ok = ok && first.index === 0 && first.input === 'ab a' && rx.lastIndex === 2; \
         var second = rx.exec('ab a'); \
         ok = ok && second !== null && second[0] === 'a' && second[1] === 'a' && second[2] === undefined; \
         ok = ok && second.index === 3 && second.input === 'ab a' && rx.lastIndex === 4; \
         ok && rx.exec('ab a') === null && rx.lastIndex === 0;",
        JsValue::Bool(true),
    );
}

#[test]
fn regexp_constructor_rejects_unsupported_flags_and_patterns() {
    assert_script(
        "var invalidFlag = false; \
         var duplicateFlag = false; \
         var invalidPattern = false; \
         try { new RegExp('a', 'z'); } catch (e) { invalidFlag = e instanceof SyntaxError; } \
         try { new RegExp('a', 'gg'); } catch (e) { duplicateFlag = e instanceof SyntaxError; } \
         try { new RegExp('('); } catch (e) { invalidPattern = e instanceof SyntaxError; } \
         invalidFlag && duplicateFlag && invalidPattern;",
        JsValue::Bool(true),
    );
}
