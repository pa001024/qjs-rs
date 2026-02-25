#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::run_script;

fn assert_script(source: &str, expected: JsValue) {
    assert_eq!(run_script(source, &[]), Ok(expected));
}

#[test]
fn direct_eval_reads_caller_lexical_binding() {
    assert_script(
        "function f() { let x = 41; return eval('x + 1'); } f();",
        JsValue::Number(42.0),
    );
}

#[test]
fn indirect_eval_reads_global_not_caller_lexical_binding() {
    assert_script(
        "var x = 7; function f() { let x = 9; return (0, eval)('x'); } f();",
        JsValue::Number(7.0),
    );
}

#[test]
fn direct_eval_non_strict_var_binds_into_caller_function_scope() {
    assert_script(
        "function f() { eval('var leaked = 5;'); return leaked; } f();",
        JsValue::Number(5.0),
    );
}

#[test]
fn direct_eval_strict_var_does_not_bind_caller_scope() {
    assert_script(
        "function f() { 'use strict'; eval('var hidden = 5;'); return typeof hidden; } f();",
        JsValue::String("undefined".to_string()),
    );
}

#[test]
fn indirect_eval_creates_global_var_binding() {
    assert_script(
        "function f() { let x = 1; (0, eval)('var __qjs_indirect_eval_global__ = 9;'); return x; } \
         f(); \
         __qjs_indirect_eval_global__;",
        JsValue::Number(9.0),
    );
}

#[test]
fn direct_eval_inherits_strict_this_binding() {
    assert_script(
        "function f() { 'use strict'; return eval('this === undefined'); } f();",
        JsValue::Bool(true),
    );
}

#[test]
fn indirect_eval_uses_global_this_binding() {
    assert_script(
        "function f() { 'use strict'; return (0, eval)('this === globalThis'); } f();",
        JsValue::Bool(true),
    );
}

#[test]
fn eval_preserves_syntax_error_category() {
    assert_script(
        "var ok = false; \
         try { eval('if ('); } catch (err) { ok = err instanceof SyntaxError; } \
         ok;",
        JsValue::Bool(true),
    );
}

#[test]
fn eval_preserves_reference_error_category() {
    assert_script(
        "var ok = false; \
         try { eval('missingEvalName'); } catch (err) { ok = err instanceof ReferenceError; } \
         ok;",
        JsValue::Bool(true),
    );
}

#[test]
fn eval_preserves_type_error_category() {
    assert_script(
        "var ok = false; \
         try { eval('null.f()'); } catch (err) { ok = err instanceof TypeError; } \
         ok;",
        JsValue::Bool(true),
    );
}

#[test]
fn eval_restores_with_scope_after_exception() {
    assert_script(
        "var scope = { marker: 1 }; \
         function f() { \
           var leaked = false; \
           with (scope) { \
             try { eval('throw 1;'); } catch (err) {} \
           } \
           try { marker; leaked = true; } catch (err) {} \
           return leaked; \
         } \
         f();",
        JsValue::Bool(false),
    );
}

#[test]
fn nested_closure_and_block_shadowing_preserve_capture() {
    assert_script(
        "function make() { \
           let value = 2; \
           return function() { \
             let acc = value; \
             { \
               let value = 3; \
               acc = acc * 10 + value; \
             } \
             return acc; \
           }; \
         } \
         make()();",
        JsValue::Number(23.0),
    );
}

#[test]
fn tdz_error_from_closure_access_before_initialization_is_reference_error() {
    assert_script(
        "let ok = false; \
         { \
           let reader = () => value; \
           try { reader(); } catch (err) { ok = err instanceof ReferenceError; } \
           let value = 1; \
         } \
         ok;",
        JsValue::Bool(true),
    );
}

#[test]
fn per_iteration_lexical_bindings_stay_distinct_with_control_flow() {
    assert_script(
        "let fns = []; \
         for (let i = 0; i < 4; i = i + 1) { \
           if (i === 2) { \
             fns.push(function() { return i; }); \
             continue; \
           } \
           fns.push(function() { return i; }); \
         } \
         fns[0]() + fns[1]() * 10 + fns[2]() * 100 + fns[3]() * 1000;",
        JsValue::Number(3210.0),
    );
}
