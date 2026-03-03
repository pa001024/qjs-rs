use runtime::{JsValue, Realm};

use crate::{Vm, VmError};

pub(super) fn execute_string_constructor(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let value = match args.first() {
        None => String::new(),
        Some(value) => vm.coerce_to_string_runtime(value.clone(), realm, caller_strict)?,
    };
    Ok(JsValue::String(value))
}

pub(super) fn execute_string_from_char_code(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let mut output = String::new();
    for value in args {
        let number = vm.coerce_number_runtime(value.clone(), realm, caller_strict)?;
        let code = Vm::to_uint32_number(number) & 0xFFFF;
        if let Some(ch) = char::from_u32(code) {
            output.push(ch);
        } else {
            output.push('\u{FFFD}');
        }
    }
    Ok(JsValue::String(output))
}
