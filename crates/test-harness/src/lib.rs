#![forbid(unsafe_code)]

use bytecode::compile_expression;
use parser::parse_expression;
use runtime::JsValue;
use vm::Vm;

pub fn run_expression(source: &str) -> Result<JsValue, String> {
    let expr = parse_expression(source).map_err(|err| err.message)?;
    let chunk = compile_expression(&expr);
    let mut vm = Vm::default();
    vm.execute(&chunk).map_err(|err| format!("{err:?}"))
}

#[cfg(test)]
mod tests {
    use super::run_expression;
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
}
