use runtime::{JsValue, Realm};

use crate::{BOXED_PRIMITIVE_VALUE_KEY, Vm, VmError};

pub(super) fn execute_object_constructor(vm: &mut Vm, args: &[JsValue]) -> JsValue {
    match args.first().cloned() {
        None | Some(JsValue::Null) | Some(JsValue::Undefined) => vm.create_object_value(),
        Some(
            value @ (JsValue::Object(_)
            | JsValue::Function(_)
            | JsValue::NativeFunction(_)
            | JsValue::HostFunction(_)),
        ) => value,
        Some(primitive @ (JsValue::Number(_) | JsValue::Bool(_) | JsValue::String(_))) => {
            vm.box_primitive_receiver(primitive)
        }
        Some(_) => vm.create_object_value(),
    }
}

pub(super) fn execute_object_create(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let (prototype, prototype_value) =
        vm.parse_prototype_value(args.first().cloned().unwrap_or(JsValue::Undefined))?;

    let object = vm.create_object_value();
    let object_id = match object {
        JsValue::Object(id) => id,
        _ => unreachable!(),
    };
    let target = vm
        .objects
        .get_mut(&object_id)
        .ok_or(VmError::UnknownObject(object_id))?;
    target.prototype = prototype;
    target.prototype_value = prototype_value;
    if !matches!(args.get(1), None | Some(JsValue::Undefined)) {
        let descriptors = vm.coerce_object_for_object_builtins(
            args.get(1).cloned().unwrap_or(JsValue::Undefined),
            "Object.create properties must be coercible",
        )?;
        let define_args = [JsValue::Object(object_id), descriptors];
        let _ = vm.execute_object_define_properties(&define_args, realm)?;
    }
    Ok(JsValue::Object(object_id))
}

pub(super) fn execute_object_assign(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let target = vm.coerce_object_for_object_builtins(
        args.first().cloned().unwrap_or(JsValue::Undefined),
        "Object.assign target must be coercible",
    )?;

    for source in args.iter().skip(1).cloned() {
        if matches!(source, JsValue::Null | JsValue::Undefined) {
            continue;
        }
        let source_object =
            vm.coerce_object_for_object_builtins(source, "Object.assign source must be coercible")?;
        let keys = collect_object_assign_keys(vm, source_object.clone(), realm)?;

        for key in keys {
            let Some(descriptor) =
                object_assign_get_own_property_descriptor(vm, source_object.clone(), &key, realm)?
            else {
                continue;
            };
            let enumerable = vm.get_property_from_receiver(descriptor, "enumerable", realm)?;
            if !vm.is_truthy(&enumerable) {
                continue;
            }
            let value = vm.get_property_from_receiver(source_object.clone(), &key, realm)?;
            vm.ensure_assign_target_writable(&target, &key)?;
            let _ = vm.set_property_on_receiver(target.clone(), key, value, realm)?;
        }
    }
    Ok(target)
}

fn collect_object_assign_keys(
    vm: &mut Vm,
    source_object: JsValue,
    realm: &Realm,
) -> Result<Vec<String>, VmError> {
    let JsValue::Object(object_id) = source_object.clone() else {
        return vm.collect_own_property_keys(&source_object, false);
    };
    let Some((proxy_target, proxy_handler)) = vm.object_proxy_slots(object_id)? else {
        return vm.collect_own_property_keys(&source_object, false);
    };
    let trap = vm.get_property_from_receiver(proxy_handler.clone(), "ownKeys", realm)?;
    if matches!(trap, JsValue::Undefined) {
        if matches!(proxy_target, JsValue::Object(target_id) if target_id == object_id) {
            return Ok(Vec::new());
        }
        return collect_object_assign_keys(vm, proxy_target, realm);
    }
    if !Vm::is_callable_value(&trap) {
        return Err(VmError::TypeError("Proxy ownKeys trap must be callable"));
    }
    let trap_result =
        vm.execute_callable(trap, Some(proxy_handler), vec![proxy_target], realm, false)?;
    if !Vm::is_object_like_value(&trap_result) {
        return Err(VmError::TypeError("Proxy ownKeys trap must return object"));
    }
    let length_value = vm.get_property_from_receiver(trap_result.clone(), "length", realm)?;
    let length_number = vm.coerce_number_runtime(length_value, realm, false)?;
    let length = Vm::to_length_from_number(length_number).max(0) as usize;
    let mut keys = Vec::with_capacity(length);
    for index in 0..length {
        let key_value =
            vm.get_property_from_receiver(trap_result.clone(), &index.to_string(), realm)?;
        let key = vm.coerce_to_property_key_runtime(key_value, realm, false)?;
        keys.push(key);
    }
    Ok(keys)
}

fn object_assign_get_own_property_descriptor(
    vm: &mut Vm,
    source_object: JsValue,
    key: &str,
    realm: &Realm,
) -> Result<Option<JsValue>, VmError> {
    let JsValue::Object(object_id) = source_object.clone() else {
        let descriptor = vm.execute_object_get_own_property_descriptor(
            &[source_object, JsValue::String(key.to_string())],
            realm,
        )?;
        return Ok((!matches!(descriptor, JsValue::Undefined)).then_some(descriptor));
    };
    let Some((proxy_target, proxy_handler)) = vm.object_proxy_slots(object_id)? else {
        let descriptor = vm.execute_object_get_own_property_descriptor(
            &[JsValue::Object(object_id), JsValue::String(key.to_string())],
            realm,
        )?;
        return Ok((!matches!(descriptor, JsValue::Undefined)).then_some(descriptor));
    };
    let trap =
        vm.get_property_from_receiver(proxy_handler.clone(), "getOwnPropertyDescriptor", realm)?;
    if matches!(trap, JsValue::Undefined) {
        if matches!(proxy_target, JsValue::Object(target_id) if target_id == object_id) {
            return Ok(None);
        }
        return object_assign_get_own_property_descriptor(vm, proxy_target, key, realm);
    }
    if !Vm::is_callable_value(&trap) {
        return Err(VmError::TypeError(
            "Proxy getOwnPropertyDescriptor trap must be callable",
        ));
    }
    let descriptor = vm.execute_callable(
        trap,
        Some(proxy_handler),
        vec![proxy_target, JsValue::String(key.to_string())],
        realm,
        false,
    )?;
    if matches!(descriptor, JsValue::Undefined) {
        return Ok(None);
    }
    if !Vm::is_object_like_value(&descriptor) {
        return Err(VmError::TypeError(
            "Proxy getOwnPropertyDescriptor trap must return object or undefined",
        ));
    }
    Ok(Some(descriptor))
}

pub(super) fn execute_object_keys(vm: &mut Vm, args: &[JsValue]) -> Result<JsValue, VmError> {
    let target = vm.coerce_object_for_object_builtins(
        args.first().cloned().unwrap_or(JsValue::Undefined),
        "Object.keys target must be object",
    )?;
    let snapshot = vm.collect_own_property_keys(&target, false)?;
    let mut keys = Vec::new();
    for key in snapshot {
        if !vm.has_own_property(&target, &key)? {
            continue;
        }
        if !vm.own_property_is_enumerable(&target, &key)? {
            continue;
        }
        keys.push(key);
    }
    vm.create_array_from_string_keys(keys)
}

pub(super) fn execute_object_entries(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let target = vm.coerce_object_for_object_builtins(
        args.first().cloned().unwrap_or(JsValue::Undefined),
        "Object.entries target must be object",
    )?;
    let snapshot = vm.collect_own_property_keys(&target, false)?;
    let mut entries = Vec::with_capacity(snapshot.len());
    for key in snapshot {
        if !vm.has_own_property(&target, &key)? {
            continue;
        }
        if !vm.own_property_is_enumerable(&target, &key)? {
            continue;
        }
        let value = vm.get_property_from_receiver(target.clone(), &key, realm)?;
        let entry = vm.create_array_from_values(vec![JsValue::String(key), value])?;
        entries.push(entry);
    }
    vm.create_array_from_values(entries)
}

pub(super) fn execute_object_values(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let target = vm.coerce_object_for_object_builtins(
        args.first().cloned().unwrap_or(JsValue::Undefined),
        "Object.values target must be object",
    )?;
    let snapshot = vm.collect_own_property_keys(&target, false)?;
    let mut values = Vec::with_capacity(snapshot.len());
    for key in snapshot {
        if !vm.has_own_property(&target, &key)? {
            continue;
        }
        if !vm.own_property_is_enumerable(&target, &key)? {
            continue;
        }
        values.push(vm.get_property_from_receiver(target.clone(), &key, realm)?);
    }
    vm.create_array_from_values(values)
}

pub(super) fn execute_object_get_own_property_names(
    vm: &mut Vm,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = vm.coerce_object_for_object_builtins(
        args.first().cloned().unwrap_or(JsValue::Undefined),
        "Object.getOwnPropertyNames target must be object",
    )?;
    let mut keys = vm.collect_own_property_keys(&target, false)?;
    keys.retain(|key| key != BOXED_PRIMITIVE_VALUE_KEY);
    vm.create_array_from_string_keys(keys)
}

pub(super) fn execute_object_get_own_property_symbols(
    vm: &mut Vm,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let _target = vm.coerce_object_for_object_builtins(
        args.first().cloned().unwrap_or(JsValue::Undefined),
        "Object.getOwnPropertySymbols target must be object",
    )?;
    vm.create_array_from_values(Vec::new())
}

pub(super) fn execute_object_define_properties(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let target = args.first().cloned().unwrap_or(JsValue::Undefined);
    if !Vm::is_object_like_value(&target) {
        return Err(VmError::TypeError(
            "Object.defineProperties target must be object",
        ));
    }
    let descriptors = vm.coerce_object_for_object_builtins(
        args.get(1).cloned().unwrap_or(JsValue::Undefined),
        "Object.defineProperties descriptors must be object",
    )?;
    let descriptor_keys = collect_object_assign_keys(vm, descriptors.clone(), realm)?;
    let mut normalized_descriptors = Vec::with_capacity(descriptor_keys.len());
    for property_name in descriptor_keys {
        let Some(own_descriptor) = object_assign_get_own_property_descriptor(
            vm,
            descriptors.clone(),
            &property_name,
            realm,
        )?
        else {
            continue;
        };
        let enumerable = vm.get_property_from_receiver(own_descriptor, "enumerable", realm)?;
        if !vm.is_truthy(&enumerable) {
            continue;
        }
        let descriptor =
            vm.get_property_from_receiver(descriptors.clone(), &property_name, realm)?;
        let parsed = vm.parse_property_descriptor(descriptor, realm)?;
        let normalized = vm.materialize_property_descriptor(&parsed);
        normalized_descriptors.push((property_name, normalized));
    }
    for (property_name, descriptor) in normalized_descriptors {
        let define_args = [target.clone(), JsValue::String(property_name), descriptor];
        let _ = vm.execute_object_define_property(&define_args, realm)?;
    }
    Ok(target)
}
