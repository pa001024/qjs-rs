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

pub(super) fn execute_object_from_entries(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let iterable = args.first().cloned().unwrap_or(JsValue::Undefined);
    let target = vm.create_object_value();
    let iterator_record = vm.create_for_of_runtime_iterator_record(iterable, realm)?;

    loop {
        let step = vm.execute_object_for_of_step(std::slice::from_ref(&iterator_record), realm)?;
        let done = vm
            .get_property_from_receiver(step.clone(), "done", realm)
            .map(|value| vm.is_truthy(&value))?;
        if done {
            break;
        }

        let next = match vm.get_property_from_receiver(step, "value", realm) {
            Ok(value) => value,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };
        if !Vm::is_object_like_value(&next) {
            let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
            return Err(VmError::TypeError(
                "Object.fromEntries iterable entries must be object",
            ));
        }

        let key = match vm.get_property_from_receiver(next.clone(), "0", realm) {
            Ok(value) => value,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };
        let value = match vm.get_property_from_receiver(next, "1", realm) {
            Ok(value) => value,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };
        let key = match vm.coerce_to_property_key_runtime(key, realm, false) {
            Ok(key) => key,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };
        if let Err(err) = vm.create_data_property_or_throw(target.clone(), key, value, realm) {
            let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
            return Err(err);
        }
    }

    Ok(target)
}

fn append_group_by_item(
    vm: &mut Vm,
    bucket: JsValue,
    value: JsValue,
    realm: &Realm,
) -> Result<(), VmError> {
    if !Vm::is_object_like_value(&bucket) {
        return Err(VmError::TypeError("Object.groupBy bucket must be object"));
    }
    let length_value = vm.get_property_from_receiver(bucket.clone(), "length", realm)?;
    let length_number = vm.coerce_number_runtime(length_value, realm, false)?;
    let length = Vm::to_length_from_number(length_number).max(0);
    vm.create_data_property_or_throw(bucket, length.to_string(), value, realm)
}

pub(super) fn execute_object_group_by(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let items = args.first().cloned().unwrap_or(JsValue::Undefined);
    let callback = args.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !Vm::is_callable_value(&callback) {
        return Err(VmError::NotCallable);
    }

    let groups = vm.create_object_value();
    let groups_id = match groups {
        JsValue::Object(id) => id,
        _ => unreachable!(),
    };
    let groups_object = vm
        .objects
        .get_mut(&groups_id)
        .ok_or(VmError::UnknownObject(groups_id))?;
    groups_object.prototype = None;
    groups_object.prototype_value = None;

    let iterator_record = match items {
        JsValue::String(value) => vm.create_for_of_snapshot_record(vm.js_string_iterator_values(&value))?,
        JsValue::Null | JsValue::Undefined => {
            return Err(VmError::TypeError("for-of expects iterable"));
        }
        value if Vm::is_object_like_value(&value) => {
            vm.create_for_of_runtime_iterator_record(value, realm)?
        }
        primitive => {
            let boxed = vm.box_primitive_receiver(primitive);
            vm.create_for_of_runtime_iterator_record(boxed, realm)?
        }
    };
    let mut index: usize = 0;
    loop {
        let step = vm.execute_object_for_of_step(std::slice::from_ref(&iterator_record), realm)?;
        let done = vm
            .get_property_from_receiver(step.clone(), "done", realm)
            .map(|value| vm.is_truthy(&value))?;
        if done {
            break;
        }

        let next = match vm.get_property_from_receiver(step, "value", realm) {
            Ok(value) => value,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };
        let key_value = match vm.execute_callable(
            callback.clone(),
            Some(JsValue::Undefined),
            vec![next.clone(), JsValue::Number(index as f64)],
            realm,
            false,
        ) {
            Ok(value) => value,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };
        let key = match vm.coerce_to_property_key_runtime(key_value, realm, false) {
            Ok(key) => key,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };

        let bucket = match vm.get_property_from_receiver(groups.clone(), &key, realm) {
            Ok(JsValue::Undefined) => {
                let array = match vm.create_array_from_values(Vec::new()) {
                    Ok(array) => array,
                    Err(err) => {
                        let _ = vm.execute_object_for_of_close(
                            std::slice::from_ref(&iterator_record),
                            realm,
                        );
                        return Err(err);
                    }
                };
                if let Err(err) =
                    vm.set_property_on_receiver(groups.clone(), key.clone(), array.clone(), realm)
                {
                    let _ = vm
                        .execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                    return Err(err);
                }
                array
            }
            Ok(existing) => existing,
            Err(err) => {
                let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
                return Err(err);
            }
        };
        if let Err(err) = append_group_by_item(vm, bucket, next, realm) {
            let _ = vm.execute_object_for_of_close(std::slice::from_ref(&iterator_record), realm);
            return Err(err);
        }
        index += 1;
    }

    Ok(groups)
}

pub(super) fn collect_object_assign_keys(
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

pub(super) fn object_assign_get_own_property_descriptor(
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

fn collect_enumerable_own_string_keys(
    vm: &mut Vm,
    target: JsValue,
    realm: &Realm,
) -> Result<Vec<String>, VmError> {
    let snapshot = collect_object_assign_keys(vm, target.clone(), realm)?;
    let mut keys = Vec::with_capacity(snapshot.len());
    for key in snapshot {
        if Vm::is_symbol_primitive_string(&key) {
            continue;
        }
        let Some(descriptor) =
            object_assign_get_own_property_descriptor(vm, target.clone(), &key, realm)?
        else {
            continue;
        };
        let enumerable = vm.get_property_from_receiver(descriptor, "enumerable", realm)?;
        if vm.is_truthy(&enumerable) {
            keys.push(key);
        }
    }
    Ok(keys)
}

fn get_property_with_proxy_trap(
    vm: &mut Vm,
    receiver: JsValue,
    property_name: &str,
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let JsValue::Object(object_id) = receiver.clone() else {
        return vm.get_property_from_receiver(receiver, property_name, realm);
    };
    let Some((proxy_target, proxy_handler)) = vm.object_proxy_slots(object_id)? else {
        return vm.get_property_from_receiver(receiver, property_name, realm);
    };
    let trap = vm.get_property_from_receiver(proxy_handler.clone(), "get", realm)?;
    if matches!(trap, JsValue::Undefined) {
        if matches!(proxy_target, JsValue::Object(target_id) if target_id == object_id) {
            return Ok(JsValue::Undefined);
        }
        return get_property_with_proxy_trap(vm, proxy_target, property_name, realm);
    }
    if !Vm::is_callable_value(&trap) {
        return Err(VmError::TypeError("Proxy get trap must be callable"));
    }
    vm.execute_callable(
        trap,
        Some(proxy_handler),
        vec![
            proxy_target,
            JsValue::String(property_name.to_string()),
            receiver,
        ],
        realm,
        false,
    )
}

pub(super) fn execute_object_keys(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let target = vm.coerce_object_for_object_builtins(
        args.first().cloned().unwrap_or(JsValue::Undefined),
        "Object.keys target must be object",
    )?;
    let keys = collect_enumerable_own_string_keys(vm, target, realm)?;
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
    let snapshot = collect_object_assign_keys(vm, target.clone(), realm)?;
    let mut entries = Vec::with_capacity(snapshot.len());
    for key in snapshot {
        if Vm::is_symbol_primitive_string(&key) {
            continue;
        }
        let Some(descriptor) =
            object_assign_get_own_property_descriptor(vm, target.clone(), &key, realm)?
        else {
            continue;
        };
        let enumerable = vm.get_property_from_receiver(descriptor, "enumerable", realm)?;
        if !vm.is_truthy(&enumerable) {
            continue;
        }
        let value = get_property_with_proxy_trap(vm, target.clone(), &key, realm)?;
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
    let snapshot = collect_object_assign_keys(vm, target.clone(), realm)?;
    let mut values = Vec::with_capacity(snapshot.len());
    for key in snapshot {
        if Vm::is_symbol_primitive_string(&key) {
            continue;
        }
        let Some(descriptor) =
            object_assign_get_own_property_descriptor(vm, target.clone(), &key, realm)?
        else {
            continue;
        };
        let enumerable = vm.get_property_from_receiver(descriptor, "enumerable", realm)?;
        if !vm.is_truthy(&enumerable) {
            continue;
        }
        values.push(get_property_with_proxy_trap(vm, target.clone(), &key, realm)?);
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
