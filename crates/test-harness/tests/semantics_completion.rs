#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::run_script;

fn assert_script_value(source: &str, expected: JsValue) {
    let result = run_script(source, &[]);
    assert_eq!(result, Ok(expected), "unexpected result for script:\n{source}");
}

#[test]
fn completion_grid_if_switch_loop_nested_paths() {
    assert_script_value(
        r#"eval("for (var i = 0; i < 3; i = i + 1) { switch (i) { case 0: if (false) { 100; } else { ; } 10; break; case 1: if (true) { 20; } else { 200; } break; default: 30; } }");"#,
        JsValue::Number(30.0),
    );
}

#[test]
fn empty_if_branch_does_not_override_prior_loop_completion() {
    assert_script_value(
        r#"eval("for (var i = 0; i < 2; i = i + 1) { if (i === 0) { 7; } else { ; } }");"#,
        JsValue::Number(7.0),
    );
}

#[test]
fn switch_empty_statement_keeps_last_non_empty_completion() {
    assert_script_value(
        r#"eval("switch (1) { case 1: 5; ; break; default: 9; }");"#,
        JsValue::Number(5.0),
    );
}

#[test]
fn labeled_break_keeps_completion_from_nested_switch_case() {
    assert_script_value(
        r#"eval("outer: { switch (1) { case 1: 42; break outer; default: 0; } 99; }");"#,
        JsValue::Number(42.0),
    );
}

#[test]
fn break_to_outer_label_through_finally_keeps_latest_value() {
    assert_script_value(
        r#"eval("outer: for (var i = 0; i < 2; i = i + 1) { try { switch (i) { case 0: 11; break; default: 22; break outer; } } finally { ; } }");"#,
        JsValue::Number(22.0),
    );
}

#[test]
fn continue_to_outer_label_runs_finally_and_stays_deterministic() {
    assert_script_value(
        "var count = 0; outer: for (var i = 0; i < 3; i = i + 1) { for (var j = 0; j < 2; j = j + 1) { try { if (j === 0) { count = count + 1; continue outer; } } finally { count = count + 10; } } } count;",
        JsValue::Number(33.0),
    );
}

#[test]
fn return_inside_try_with_switch_survives_empty_finally() {
    assert_script_value(
        "function f(v) { try { switch (v) { case 0: return 1; default: return 2; } } finally { ; } } f(1);",
        JsValue::Number(2.0),
    );
}

#[test]
fn throw_in_finally_overrides_break_with_typed_error() {
    assert_script_value(
        "var ok = false; try { while (1) { try { break; } finally { throw new RangeError('boom'); } } } catch (e) { ok = e instanceof RangeError; } ok;",
        JsValue::Bool(true),
    );
}

#[test]
fn throw_through_finally_preserves_type_error_surface() {
    assert_script_value(
        "var ok = false; try { while (1) { try { throw new TypeError('x'); } finally { ; } } } catch (e) { ok = e instanceof TypeError; } ok;",
        JsValue::Bool(true),
    );
}

#[test]
fn continue_path_does_not_poison_later_iteration_completion() {
    assert_script_value(
        r#"eval("for (var i = 0; i < 3; i = i + 1) { try { if (i === 1) { 100; continue; } i; } finally { ; } }");"#,
        JsValue::Number(2.0),
    );
}

#[test]
fn break_from_try_finally_in_label_preserves_previous_value() {
    assert_script_value(
        r#"eval("outer: { 1; try { break outer; } finally { ; } }");"#,
        JsValue::Number(1.0),
    );
}
