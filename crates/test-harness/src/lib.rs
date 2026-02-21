#![forbid(unsafe_code)]

pub mod test262;

use builtins::install_baseline;
use bytecode::{compile_expression, compile_script};
use parser::{parse_expression, parse_script};
use runtime::{JsValue, Realm};
use vm::Vm;

pub fn run_expression(source: &str) -> Result<JsValue, String> {
    run_expression_with_globals(source, &[])
}

pub fn run_expression_with_globals(
    source: &str,
    globals: &[(&str, JsValue)],
) -> Result<JsValue, String> {
    let expr = parse_expression(source).map_err(|err| err.message)?;
    let chunk = compile_expression(&expr);
    execute_chunk_with_globals(&chunk, globals)
}

pub fn run_script(source: &str, globals: &[(&str, JsValue)]) -> Result<JsValue, String> {
    let script = parse_script(source).map_err(|err| err.message)?;
    let chunk = compile_script(&script);
    execute_chunk_with_globals(&chunk, globals)
}

fn execute_chunk_with_globals(
    chunk: &bytecode::Chunk,
    globals: &[(&str, JsValue)],
) -> Result<JsValue, String> {
    let mut realm = Realm::default();
    install_baseline(&mut realm);
    for (name, value) in globals {
        realm.define_global(name, value.clone());
    }
    let mut vm = Vm::default();
    vm.execute_in_realm(chunk, &realm)
        .map_err(|err| format!("{err:?}"))
}

#[cfg(test)]
mod tests {
    use super::{run_expression, run_expression_with_globals, run_script};
    use runtime::JsValue;

    #[test]
    fn evaluates_number_literal() {
        assert_eq!(run_expression("1"), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn evaluates_add_expression() {
        assert_eq!(run_expression("1 + 2 + 3"), Ok(JsValue::Number(6.0)));
    }

    #[test]
    fn evaluates_operator_precedence() {
        assert_eq!(run_expression("1 + 2 * 3"), Ok(JsValue::Number(7.0)));
        assert_eq!(run_expression("(1 + 2) * 3"), Ok(JsValue::Number(9.0)));
    }

    #[test]
    fn evaluates_sub_and_div() {
        assert_eq!(run_expression("10 - 4 / 2"), Ok(JsValue::Number(8.0)));
    }

    #[test]
    fn evaluates_basic_numeric_coercion() {
        assert_eq!(run_expression("'2' * 3"), Ok(JsValue::Number(6.0)));

        let value = run_expression("1 * {}").expect("expression should execute");
        match value {
            JsValue::Number(number) => assert!(number.is_nan()),
            other => panic!("expected Number(NaN), got {other:?}"),
        }
    }

    #[test]
    fn evaluates_unary_operators() {
        assert_eq!(run_expression("-5 + +2"), Ok(JsValue::Number(-3.0)));
        assert_eq!(run_expression("!0"), Ok(JsValue::Bool(true)));
        assert_eq!(
            run_expression("typeof 1"),
            Ok(JsValue::String("number".to_string()))
        );
        assert_eq!(run_expression("void 1"), Ok(JsValue::Undefined));
        assert_eq!(run_expression("delete x"), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn installs_baseline_globals() {
        assert_eq!(
            run_expression("typeof eval"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof Function"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof Object"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof Number"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof String"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof isNaN"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof assert"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof Test262Error"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof TypeError"),
            Ok(JsValue::String("function".to_string()))
        );
        assert_eq!(
            run_expression("typeof globalThis"),
            Ok(JsValue::String("object".to_string()))
        );
        assert_eq!(
            run_script("globalThis === this;", &[]),
            Ok(JsValue::Bool(true))
        );
    }

    #[test]
    fn evaluates_eval_string_source() {
        assert_eq!(run_script("eval('1 + 2');", &[]), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn supports_function_constructor_baseline() {
        let result = run_script(
            "var add = Function('a', 'b', 'return a + b;'); add(20, 22);",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn supports_number_constructor_baseline_properties() {
        let result = run_script("Number.NaN !== Number.NaN;", &[]);
        assert_eq!(result, Ok(JsValue::Bool(true)));
    }

    #[test]
    fn supports_string_and_isnan_baseline() {
        assert_eq!(
            run_script("String(123);", &[]),
            Ok(JsValue::String("123".to_string()))
        );
        assert_eq!(
            run_script("isNaN(Number.NaN);", &[]),
            Ok(JsValue::Bool(true))
        );
    }

    #[test]
    fn parses_postfix_update_expressions_in_non_strict_mode() {
        let result = run_script("if (false) { arguments++; } eval--; 1;", &[]);
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn supports_function_call_apply_bind_baseline() {
        assert_eq!(
            run_script(
                "function id() { return this; } var o = {}; id.call(o) === o;",
                &[]
            ),
            Ok(JsValue::Bool(true))
        );
        assert_eq!(
            run_script(
                "function add(a, b) { return a + b; } add.apply(null, [20, 22]);",
                &[],
            ),
            Ok(JsValue::Number(42.0))
        );
        assert_eq!(
            run_script(
                "function add(a, b) { return a + b; } var add20 = add.bind(null, 20); add20(22);",
                &[],
            ),
            Ok(JsValue::Number(42.0))
        );
    }

    #[test]
    fn supports_test262_assert_baseline() {
        let result = run_script(
            "assert(true); assert.sameValue(NaN, NaN); assert.notSameValue(0, -0); 1;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn supports_test262_assert_throws_baseline() {
        let result = run_script(
            "assert.throws(Test262Error, function() { throw new Test262Error('x'); }); 1;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn supports_test262_assert_throws_for_vm_errors() {
        let result = run_script(
            "assert.throws(TypeError, function() { var v = 1; v.x; }); 1;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn supports_object_define_property_accessor_baseline() {
        assert_eq!(
            run_script(
                "var o = {}; Object.defineProperty(o, 'foo', { get: function() { return this; } }); o.foo === o;",
                &[],
            ),
            Ok(JsValue::Bool(true))
        );
        assert_eq!(
            run_script(
                "var x = null; var o = {}; Object.defineProperty(o, 'foo', { set: function(v) { x = this; } }); o.foo = 1; x === o;",
                &[],
            ),
            Ok(JsValue::Bool(true))
        );
    }

    #[test]
    fn supports_object_literal_accessor_this_binding() {
        assert_eq!(
            run_script("var o = { get foo() { return this; } }; o.foo === o;", &[]),
            Ok(JsValue::Bool(true))
        );
        assert_eq!(
            run_script(
                "var x = null; var o = { set foo(v) { x = this; } }; o.foo = 1; x === o;",
                &[],
            ),
            Ok(JsValue::Bool(true))
        );
    }

    #[test]
    fn supports_object_literal_computed_accessor_baseline() {
        assert_eq!(
            run_script(
                "var k = 'foo'; var o = { get [k]() { return this; } }; o.foo === o;",
                &[],
            ),
            Ok(JsValue::Bool(true))
        );
        assert_eq!(
            run_script(
                "var k = 'foo'; var x = null; var o = { set [k](v) { x = this; } }; o.foo = 1; x === o;",
                &[],
            ),
            Ok(JsValue::Bool(true))
        );
    }

    #[test]
    fn delete_member_in_getter_does_not_recurse() {
        let result = run_script(
            "var o = { get x() { delete this.x; return 1; } }; var a = o.x; var b = o.x; (a === 1) && (b === undefined);",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Bool(true)));
    }

    #[test]
    fn supports_unicode_identifier_escapes() {
        let result = run_script("var \\u0061 = 41; a + 1;", &[]);
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn supports_unicode_codepoint_identifier_escapes() {
        let result = run_script("var _\\u{1F600} = 41; _😀 + 1;", &[]);
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn supports_string_replace_callback_baseline() {
        let result = run_script(
            "var x = 1; var out = 'ab'.replace('b', function() { x = this; return 'a'; }); (out === 'aa') && (typeof x === 'object') && (x !== null);",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Bool(true)));
    }

    #[test]
    fn supports_string_replace_callback_strict_this() {
        let result = run_script(
            "var x = 1; var out = 'ab'.replace('b', function() { 'use strict'; x = this; return 'a'; }); (out === 'aa') && (x === undefined);",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Bool(true)));
    }

    #[test]
    fn parses_regex_literal_expression_baseline() {
        let result = run_script("typeof /x/g;", &[]);
        assert_eq!(result, Ok(JsValue::String("object".to_string())));
    }

    #[test]
    fn supports_new_expression_baseline() {
        let result = run_script("function F() { this.x = 1; } var o = new F(); o.x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn supports_regexp_constructor_baseline() {
        let result = run_script("var a = new RegExp('x', 'g'); a.source + a.flags;", &[]);
        assert_eq!(result, Ok(JsValue::String("xg".to_string())));
    }

    #[test]
    fn evaluates_boolean_and_null_literals() {
        assert_eq!(run_expression("true"), Ok(JsValue::Bool(true)));
        assert_eq!(run_expression("false"), Ok(JsValue::Bool(false)));
        assert_eq!(run_expression("null"), Ok(JsValue::Null));
        assert_eq!(
            run_expression("'ok'"),
            Ok(JsValue::String("ok".to_string()))
        );
        assert_eq!(run_expression("!null"), Ok(JsValue::Bool(true)));
        assert_eq!(run_expression("!''"), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn evaluates_string_concatenation_with_add_operator() {
        assert_eq!(
            run_expression("'qjs' + 1"),
            Ok(JsValue::String("qjs1".to_string()))
        );
    }

    #[test]
    fn evaluates_comparison_operators() {
        assert_eq!(run_expression("1 + 2 * 3 >= 7"), Ok(JsValue::Bool(true)));
        assert_eq!(run_expression("3 == 4"), Ok(JsValue::Bool(false)));
        assert_eq!(run_expression("3 != 4"), Ok(JsValue::Bool(true)));
        assert_eq!(run_expression("3 === 3"), Ok(JsValue::Bool(true)));
        assert_eq!(run_expression("3 !== 4"), Ok(JsValue::Bool(true)));
        assert_eq!(
            run_script("var o = { x: 1 }; 'x' in o;", &[]),
            Ok(JsValue::Bool(true))
        );
    }

    #[test]
    fn evaluates_logical_operators_with_short_circuit() {
        assert_eq!(run_expression("0 && 1"), Ok(JsValue::Number(0.0)));
        assert_eq!(run_expression("1 && 2"), Ok(JsValue::Number(2.0)));
        assert_eq!(run_expression("0 || 2"), Ok(JsValue::Number(2.0)));
        assert_eq!(run_expression("1 || 2"), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn surfaces_unknown_identifier_error() {
        let result = run_expression("foo + 1");
        assert!(result.is_err());
        assert!(
            result
                .expect_err("identifier lookup should fail")
                .contains("UnknownIdentifier")
        );
    }

    #[test]
    fn resolves_identifiers_from_globals() {
        let result = run_expression_with_globals("foo * 2 + 1", &[("foo", JsValue::Number(20.0))]);
        assert_eq!(result, Ok(JsValue::Number(41.0)));
    }

    #[test]
    fn evaluates_let_const_and_assignment_script() {
        let result = run_script("let x = 1; const y = 2; x = x + y; x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn evaluates_var_declaration_script() {
        let result = run_script("var x = 1; x = x + 2; x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn evaluates_var_declaration_list_script() {
        let result = run_script("var x, y = 2; x = 1; x + y;", &[]);
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn allows_let_identifier_in_non_strict_var_and_object_shorthand() {
        let result = run_script("var let = 1; var object = {let}; object.let;", &[]);
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn short_circuit_skips_rhs_side_effects() {
        let result = run_script("let x = 0; 0 && (x = 1); 1 || (x = 2); x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(0.0)));
    }

    #[test]
    fn rejects_assignment_to_const_in_script() {
        let err = run_script("const x = 1; x = 2; x;", &[]).expect_err("script should fail");
        assert!(err.contains("ImmutableBinding"));
    }

    #[test]
    fn supports_block_shadowing() {
        let result = run_script("let x = 1; { let x = 2; x = x + 1; }; x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn supports_outer_assignment_inside_block() {
        let result = run_script("let x = 1; { x = x + 2; } x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn returns_undefined_when_script_has_no_result_expression() {
        let result = run_script("let x = 1;", &[]);
        assert_eq!(result, Ok(JsValue::Undefined));
    }

    #[test]
    fn executes_function_declaration_and_call() {
        let result = run_script("function add(a, b) { return a + b; } add(20, 22);", &[]);
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn function_call_can_observe_outer_binding() {
        let result = run_script("let x = 10; function add(v) { return x + v; } add(1);", &[]);
        assert_eq!(result, Ok(JsValue::Number(11.0)));
    }

    #[test]
    fn function_call_observes_latest_outer_assignment() {
        let result = run_script(
            "let x = 10; function add(v) { return x + v; } x = 20; add(1);",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(21.0)));
    }

    #[test]
    fn function_has_arguments_object() {
        let result = run_script(
            "function f(a, b) { return arguments[0] + arguments[1]; } f(20, 22);",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn arrow_function_can_return_object_for_computed_member_assignment() {
        let result = run_script(
            "let v = 'v'; let o = { [v]: 1, f() {} }; let f = () => o; f()[v] = 2; o[v];",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn parses_and_executes_call_with_spread_and_trailing_comma_shape() {
        let result = run_script("function foo() {} foo(...[],);", &[]);
        assert_eq!(result, Ok(JsValue::Undefined));
    }

    #[test]
    fn parses_object_accessor_syntax_in_sloppy_mode() {
        let result = run_script("void { get foo() {}, set foo(v) {} };", &[]);
        assert_eq!(result, Ok(JsValue::Undefined));
    }

    #[test]
    fn arguments_object_exposes_length() {
        let result = run_script(
            "function f(a, b, c) { return arguments.length; } f(1, 2, 3);",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn function_assignment_updates_outer_binding() {
        let result = run_script(
            "let x = 10; function inc() { x = x + 1; return x; } inc(); x;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(11.0)));
    }

    #[test]
    fn supports_function_hoisting_at_script_scope() {
        let result = run_script("add(20, 22); function add(a, b) { return a + b; }", &[]);
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn supports_function_hoisting_inside_block() {
        let result = run_script(
            "let y = 0; { y = id(7); function id(v) { return v; } } y;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn evaluates_if_else_statement() {
        let result = run_script("let x = 0; if (x < 1) x = 2; else x = 3; x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn evaluates_terminal_if_without_else_as_undefined_when_false() {
        let result = run_script("if (0) 1;", &[]);
        assert_eq!(result, Ok(JsValue::Undefined));
    }

    #[test]
    fn evaluates_while_statement() {
        let result = run_script("let x = 0; while (x < 3) x = x + 1; x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn evaluates_do_while_statement() {
        let result = run_script("let x = 0; do { x = x + 1; } while (0); x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn evaluates_terminal_while_as_undefined() {
        let result = run_script("while (0) { }", &[]);
        assert_eq!(result, Ok(JsValue::Undefined));
    }

    #[test]
    fn evaluates_for_statement() {
        let result = run_script(
            "let sum = 0; for (let i = 0; i < 4; i = i + 1) sum = sum + i; sum;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(6.0)));
    }

    #[test]
    fn evaluates_break_statement() {
        let result = run_script(
            "let i = 0; while (1) { i = i + 1; if (i == 3) break; } i;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn evaluates_continue_statement() {
        let result = run_script(
            "let sum = 0; for (let i = 0; i < 5; i = i + 1) { if (i == 2) continue; sum = sum + i; } sum;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(8.0)));
    }

    #[test]
    fn evaluates_continue_with_nested_block_scope() {
        let result = run_script(
            "let count = 0; for (let i = 0; i < 3; i = i + 1) { { if (i == 1) continue; } count = count + 1; } count;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn evaluates_switch_statement() {
        let result = run_script(
            "let y = 0; switch (2) { case 1: y = 1; break; case 2: y = 2; break; default: y = 3; } y;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn evaluates_switch_fallthrough() {
        let result = run_script(
            "let y = 0; switch (1) { case 1: y = y + 1; case 2: y = y + 2; break; default: y = y + 4; } y;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn evaluates_switch_default_branch() {
        let result = run_script(
            "let y = 0; switch (9) { case 1: y = 1; break; default: y = 5; } y;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(5.0)));
    }

    #[test]
    fn break_in_switch_does_not_break_outer_loop() {
        let result = run_script(
            "let c = 0; for (let i = 0; i < 3; i = i + 1) { switch (i) { case 1: break; default: c = c + 1; } } c;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn continue_inside_switch_targets_outer_loop() {
        let result = run_script(
            "let sum = 0; for (let i = 0; i < 4; i = i + 1) { switch (i) { case 2: continue; default: sum = sum + i; } } sum;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(4.0)));
    }

    #[test]
    fn evaluates_try_catch_throw() {
        let result = run_script("let x = 0; try { throw 42; } catch (e) { x = e; } x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn evaluates_try_catch_through_function_call() {
        let result = run_script(
            "let x = 0; function fail() { throw 7; } try { fail(); } catch (e) { x = e; } x;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn propagates_uncaught_throw_error() {
        let err = run_script("throw 5;", &[]).expect_err("script should fail");
        assert!(err.contains("UncaughtException"));
    }

    #[test]
    fn evaluates_try_finally_side_effect() {
        let result = run_script("let x = 0; try { x = 1; } finally { x = 2; } x;", &[]);
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn return_runs_finally_and_preserves_value_without_override() {
        let result = run_script(
            "function f() { try { return 1; } finally { let x = 0; x = x + 1; } } f();",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn return_in_finally_overrides_prior_return() {
        let result = run_script(
            "function f() { try { return 1; } finally { return 2; } } f();",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn break_runs_finally_before_loop_exit() {
        let result = run_script(
            "let x = 0; while (1) { try { x = 1; break; } finally { x = 2; } } x;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn continue_runs_finally_before_next_iteration() {
        let result = run_script(
            "let x = 0; for (let i = 0; i < 3; i = i + 1) { try { if (i == 1) continue; x = x + 1; } finally { x = x + 10; } } x;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(32.0)));
    }

    #[test]
    fn return_in_finally_overrides_throw() {
        let result = run_script(
            "function f() { try { throw 1; } finally { return 9; } } f();",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(9.0)));
    }

    #[test]
    fn function_can_reference_itself() {
        let result = run_script("function f() { return f; } f();", &[]);
        assert!(matches!(result, Ok(JsValue::Function(_))));
    }

    #[test]
    fn evaluates_object_literal_and_member_access() {
        let result = run_script(
            "let key = 1; let obj = { answer: 42, key }; obj.answer;",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn evaluates_array_literal_and_index_access() {
        let result = run_script("let arr = [1, 2, 3]; arr[0] + arr[1] + arr[2];", &[]);
        assert_eq!(result, Ok(JsValue::Number(6.0)));
    }

    #[test]
    fn evaluates_array_length_property() {
        let result = run_script("let arr = [1, 2, 3]; arr.length;", &[]);
        assert_eq!(result, Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn evaluates_member_assignment_expression() {
        let result = run_script("let obj = {}; obj.value = 7; obj.value;", &[]);
        assert_eq!(result, Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn evaluates_computed_member_assignment_expression() {
        let result = run_script(
            "let obj = {}; let key = 'value'; obj[key] = 7; obj[key];",
            &[],
        );
        assert_eq!(result, Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn evaluates_labeled_statement() {
        let result = run_script("label: 1; 2;", &[]);
        assert_eq!(result, Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn supports_break_to_label_baseline() {
        assert_eq!(
            run_script("let x = 0; outer: { x = 1; break outer; x = 2; } x;", &[]),
            Ok(JsValue::Number(1.0))
        );
        assert_eq!(
            run_script(
                "let x = 0; outer: while (1) { while (1) { x = 1; break outer; } x = 2; } x;",
                &[]
            ),
            Ok(JsValue::Number(1.0))
        );
    }
}
