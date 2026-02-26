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
fn error_and_native_subclass_default_name_message_are_deterministic() {
    assert_script_bool(
        "var err = Error(); \
         var undefErr = Error(undefined); \
         var typeErr = TypeError(); \
         err.name === 'Error' && err.message === '' && err.toString() === 'Error' && \
         undefErr.message === '' && undefErr.toString() === 'Error' && \
         typeErr.name === 'TypeError' && typeErr.message === '' && typeErr.toString() === 'TypeError';",
    );
}

#[test]
fn native_error_overrides_name_and_message_follow_error_to_string_semantics() {
    assert_script_bool(
        "var err = new ReferenceError('boom'); \
         var base = err.toString() === 'ReferenceError: boom'; \
         err.name = ''; \
         var emptyName = err.toString() === 'boom'; \
         err.name = 'X'; \
         err.message = ''; \
         var emptyMessage = err.toString() === 'X'; \
         err.message = 'restored'; \
         var restored = err.toString() === 'X: restored'; \
         base && emptyName && emptyMessage && restored;",
    );
}

#[test]
fn native_error_instanceof_chain_covers_subclass_and_error_ancestor() {
    assert_script_bool(
        "var e1 = new TypeError('x'); \
         var e2 = new URIError('x'); \
         var e3 = new EvalError('x'); \
         (e1 instanceof TypeError) && (e1 instanceof Error) && \
         (e2 instanceof URIError) && (e2 instanceof Error) && \
         (e3 instanceof EvalError) && (e3 instanceof Error);",
    );
}

#[test]
fn error_to_string_object_coercion_and_receiver_guard_are_deterministic() {
    assert_script_bool(
        "var custom = { name: 'CustomError', message: 'details' }; \
         var customOk = Error.prototype.toString.call(custom) === 'CustomError: details'; \
         var threw = false; \
         try { Error.prototype.toString.call(1); } catch (e) { threw = e instanceof TypeError; } \
         customOk && threw;",
    );
}
