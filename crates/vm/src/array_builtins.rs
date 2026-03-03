use runtime::{JsValue, NativeFunction};

use crate::{ARRAY_OBJECT_MARKER_KEY, Vm, VmError};

pub(super) fn execute_array_is_array(vm: &Vm, args: &[JsValue]) -> JsValue {
    let is_array = match args.first() {
        Some(JsValue::Object(object_id)) => vm
            .has_object_marker(*object_id, ARRAY_OBJECT_MARKER_KEY)
            .unwrap_or(false),
        _ => false,
    };
    JsValue::Bool(is_array)
}

pub(super) fn execute_array_constructor(vm: &mut Vm, args: &[JsValue]) -> Result<JsValue, VmError> {
    let array = vm.create_array_value();
    let object_id = match array {
        JsValue::Object(id) => id,
        _ => unreachable!(),
    };

    let object = vm
        .objects
        .get_mut(&object_id)
        .expect("array object should exist");

    if args.len() == 1 {
        match args.first() {
            Some(JsValue::Number(length)) if length.is_finite() && *length >= 0.0 => {
                let int_length = length.floor();
                if int_length == *length && int_length <= u32::MAX as f64 {
                    Vm::set_object_array_length(object, int_length as usize);
                    return Ok(JsValue::Object(object_id));
                }
                return Err(VmError::UncaughtException(vm.create_error_exception(
                    NativeFunction::RangeErrorConstructor,
                    "RangeError",
                    "invalid array length".to_string(),
                )));
            }
            Some(JsValue::Number(_)) => {
                return Err(VmError::UncaughtException(vm.create_error_exception(
                    NativeFunction::RangeErrorConstructor,
                    "RangeError",
                    "invalid array length".to_string(),
                )));
            }
            Some(value) => {
                object.properties.insert("0".to_string(), value.clone());
                Vm::set_object_array_length(object, 1);
                return Ok(JsValue::Object(object_id));
            }
            None => return Ok(JsValue::Object(object_id)),
        }
    }

    for (index, value) in args.iter().enumerate() {
        object.properties.insert(index.to_string(), value.clone());
    }
    Vm::set_object_array_length(object, args.len());
    Ok(JsValue::Object(object_id))
}
