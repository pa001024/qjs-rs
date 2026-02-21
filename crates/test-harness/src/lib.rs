#![forbid(unsafe_code)]

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
}
