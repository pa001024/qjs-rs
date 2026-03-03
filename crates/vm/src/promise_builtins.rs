use runtime::{JsValue, Realm};
use std::cell::RefCell;
use std::rc::Rc;

use crate::{NoopPromiseJobHostHooks, ObjectId, PromiseSettlement, PropertyAttributes, Vm, VmError};

#[derive(Debug, Clone)]
struct PromiseAllState {
    result_promise_id: ObjectId,
    values: Vec<Option<JsValue>>,
    remaining: usize,
    settled: bool,
}

#[derive(Debug, Clone)]
struct PromiseAnyState {
    result_promise_id: ObjectId,
    reasons: Vec<Option<JsValue>>,
    remaining: usize,
    settled: bool,
}

#[derive(Debug, Clone)]
struct PromiseAllSettledState {
    result_promise_id: ObjectId,
    values: Vec<Option<JsValue>>,
    remaining: usize,
    settled: bool,
}

#[derive(Debug, Clone)]
struct PromiseRaceState {
    result_promise_id: ObjectId,
    settled: bool,
}

pub(super) fn execute_promise_resolve(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let value = args.first().cloned().unwrap_or(JsValue::Undefined);
    promise_resolve_value(vm, value, realm, caller_strict)
}

pub(super) fn execute_promise_reject(vm: &mut Vm, args: &[JsValue]) -> Result<JsValue, VmError> {
    let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
    vm.create_async_settled_promise(false, reason)
}

pub(super) fn execute_promise_all(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let values = collect_promise_iterable_values(
        vm,
        args.first().cloned().unwrap_or(JsValue::Undefined),
        realm,
        caller_strict,
        "Promise.all input must be iterable",
    )?;
    let result_promise_id = vm.create_pending_promise()?;
    if values.is_empty() {
        let aggregate = vm.create_array_from_values(Vec::new())?;
        let mut hooks = NoopPromiseJobHostHooks;
        vm.settle_promise_with_hooks(
            result_promise_id,
            PromiseSettlement::Fulfilled(aggregate),
            &mut hooks,
        )?;
        return Ok(JsValue::Object(result_promise_id));
    }

    let state = Rc::new(RefCell::new(PromiseAllState {
        result_promise_id,
        values: vec![None; values.len()],
        remaining: values.len(),
        settled: false,
    }));

    for (index, value) in values.into_iter().enumerate() {
        let promise = promise_resolve_value(vm, value, realm, caller_strict)?;
        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_all_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                let settle_payload = {
                    let mut state = fulfilled_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    if state.values[index].is_none() {
                        state.values[index] = Some(value);
                        state.remaining = state.remaining.saturating_sub(1);
                    }
                    if state.remaining == 0 {
                        state.settled = true;
                        let values = state
                            .values
                            .iter()
                            .map(|entry| entry.clone().unwrap_or(JsValue::Undefined))
                            .collect::<Vec<_>>();
                        Some((state.result_promise_id, values))
                    } else {
                        None
                    }
                };
                if let Some((promise_id, values)) = settle_payload {
                    let aggregate = vm.create_array_from_values(values)?;
                    let mut hooks = NoopPromiseJobHostHooks;
                    vm.settle_promise_with_hooks(
                        promise_id,
                        PromiseSettlement::Fulfilled(aggregate),
                        &mut hooks,
                    )?;
                }
                Ok(JsValue::Undefined)
            },
        );

        let rejected_state = Rc::clone(&state);
        let on_rejected = vm.register_host_callback_function(
            "__qjs_promise_all_rejected__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
                let promise_id = {
                    let mut state = rejected_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.result_promise_id
                };
                let mut hooks = NoopPromiseJobHostHooks;
                vm.settle_promise_with_hooks(
                    promise_id,
                    PromiseSettlement::Rejected(reason),
                    &mut hooks,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        let then_args = vec![promise, on_fulfilled, on_rejected];
        let _ = vm.execute_promise_then(&then_args, realm, caller_strict)?;
    }
    Ok(JsValue::Object(result_promise_id))
}

pub(super) fn execute_promise_any(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let values = collect_promise_iterable_values(
        vm,
        args.first().cloned().unwrap_or(JsValue::Undefined),
        realm,
        caller_strict,
        "Promise.any input must be iterable",
    )?;
    let result_promise_id = vm.create_pending_promise()?;
    if values.is_empty() {
        let aggregate = vm.create_array_from_values(Vec::new())?;
        let mut hooks = NoopPromiseJobHostHooks;
        vm.settle_promise_with_hooks(
            result_promise_id,
            PromiseSettlement::Rejected(aggregate),
            &mut hooks,
        )?;
        return Ok(JsValue::Object(result_promise_id));
    }

    let state = Rc::new(RefCell::new(PromiseAnyState {
        result_promise_id,
        reasons: vec![None; values.len()],
        remaining: values.len(),
        settled: false,
    }));

    for (index, value) in values.into_iter().enumerate() {
        let promise = promise_resolve_value(vm, value, realm, caller_strict)?;
        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_any_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                let promise_id = {
                    let mut state = fulfilled_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.result_promise_id
                };
                let mut hooks = NoopPromiseJobHostHooks;
                vm.settle_promise_with_hooks(
                    promise_id,
                    PromiseSettlement::Fulfilled(value),
                    &mut hooks,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        let rejected_state = Rc::clone(&state);
        let on_rejected = vm.register_host_callback_function(
            "__qjs_promise_any_rejected__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
                let settle_payload = {
                    let mut state = rejected_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    if state.reasons[index].is_none() {
                        state.reasons[index] = Some(reason);
                        state.remaining = state.remaining.saturating_sub(1);
                    }
                    if state.remaining == 0 {
                        state.settled = true;
                        let reasons = state
                            .reasons
                            .iter()
                            .map(|entry| entry.clone().unwrap_or(JsValue::Undefined))
                            .collect::<Vec<_>>();
                        Some((state.result_promise_id, reasons))
                    } else {
                        None
                    }
                };
                if let Some((promise_id, reasons)) = settle_payload {
                    let aggregate = vm.create_array_from_values(reasons)?;
                    let mut hooks = NoopPromiseJobHostHooks;
                    vm.settle_promise_with_hooks(
                        promise_id,
                        PromiseSettlement::Rejected(aggregate),
                        &mut hooks,
                    )?;
                }
                Ok(JsValue::Undefined)
            },
        );

        let then_args = vec![promise, on_fulfilled, on_rejected];
        let _ = vm.execute_promise_then(&then_args, realm, caller_strict)?;
    }
    Ok(JsValue::Object(result_promise_id))
}

pub(super) fn execute_promise_race(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let values = collect_promise_iterable_values(
        vm,
        args.first().cloned().unwrap_or(JsValue::Undefined),
        realm,
        caller_strict,
        "Promise.race input must be iterable",
    )?;
    let result_promise_id = vm.create_pending_promise()?;
    if values.is_empty() {
        return Ok(JsValue::Object(result_promise_id));
    }
    let state = Rc::new(RefCell::new(PromiseRaceState {
        result_promise_id,
        settled: false,
    }));
    for value in values {
        let promise = promise_resolve_value(vm, value, realm, caller_strict)?;
        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_race_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                let promise_id = {
                    let mut state = fulfilled_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.result_promise_id
                };
                let mut hooks = NoopPromiseJobHostHooks;
                vm.settle_promise_with_hooks(
                    promise_id,
                    PromiseSettlement::Fulfilled(value),
                    &mut hooks,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        let rejected_state = Rc::clone(&state);
        let on_rejected = vm.register_host_callback_function(
            "__qjs_promise_race_rejected__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
                let promise_id = {
                    let mut state = rejected_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.result_promise_id
                };
                let mut hooks = NoopPromiseJobHostHooks;
                vm.settle_promise_with_hooks(
                    promise_id,
                    PromiseSettlement::Rejected(reason),
                    &mut hooks,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        let then_args = vec![promise, on_fulfilled, on_rejected];
        let _ = vm.execute_promise_then(&then_args, realm, caller_strict)?;
    }
    Ok(JsValue::Object(result_promise_id))
}

pub(super) fn execute_promise_all_settled(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let values = collect_promise_iterable_values(
        vm,
        args.first().cloned().unwrap_or(JsValue::Undefined),
        realm,
        caller_strict,
        "Promise.allSettled input must be iterable",
    )?;
    let result_promise_id = vm.create_pending_promise()?;
    if values.is_empty() {
        let aggregate = vm.create_array_from_values(Vec::new())?;
        let mut hooks = NoopPromiseJobHostHooks;
        vm.settle_promise_with_hooks(
            result_promise_id,
            PromiseSettlement::Fulfilled(aggregate),
            &mut hooks,
        )?;
        return Ok(JsValue::Object(result_promise_id));
    }

    let state = Rc::new(RefCell::new(PromiseAllSettledState {
        result_promise_id,
        values: vec![None; values.len()],
        remaining: values.len(),
        settled: false,
    }));

    for (index, value) in values.into_iter().enumerate() {
        let promise = promise_resolve_value(vm, value, realm, caller_strict)?;
        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_all_settled_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                let entry = create_promise_all_settled_entry(vm, true, value)?;
                let settle_payload = {
                    let mut state = fulfilled_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    if state.values[index].is_none() {
                        state.values[index] = Some(entry);
                        state.remaining = state.remaining.saturating_sub(1);
                    }
                    if state.remaining == 0 {
                        state.settled = true;
                        let values = state
                            .values
                            .iter()
                            .map(|entry| entry.clone().unwrap_or(JsValue::Undefined))
                            .collect::<Vec<_>>();
                        Some((state.result_promise_id, values))
                    } else {
                        None
                    }
                };
                if let Some((promise_id, values)) = settle_payload {
                    let aggregate = vm.create_array_from_values(values)?;
                    let mut hooks = NoopPromiseJobHostHooks;
                    vm.settle_promise_with_hooks(
                        promise_id,
                        PromiseSettlement::Fulfilled(aggregate),
                        &mut hooks,
                    )?;
                }
                Ok(JsValue::Undefined)
            },
        );

        let rejected_state = Rc::clone(&state);
        let on_rejected = vm.register_host_callback_function(
            "__qjs_promise_all_settled_rejected__",
            1.0,
            false,
            move |vm, _this_arg, args, _realm, _strict| {
                let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
                let entry = create_promise_all_settled_entry(vm, false, reason)?;
                let settle_payload = {
                    let mut state = rejected_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    if state.values[index].is_none() {
                        state.values[index] = Some(entry);
                        state.remaining = state.remaining.saturating_sub(1);
                    }
                    if state.remaining == 0 {
                        state.settled = true;
                        let values = state
                            .values
                            .iter()
                            .map(|entry| entry.clone().unwrap_or(JsValue::Undefined))
                            .collect::<Vec<_>>();
                        Some((state.result_promise_id, values))
                    } else {
                        None
                    }
                };
                if let Some((promise_id, values)) = settle_payload {
                    let aggregate = vm.create_array_from_values(values)?;
                    let mut hooks = NoopPromiseJobHostHooks;
                    vm.settle_promise_with_hooks(
                        promise_id,
                        PromiseSettlement::Fulfilled(aggregate),
                        &mut hooks,
                    )?;
                }
                Ok(JsValue::Undefined)
            },
        );

        let then_args = vec![promise, on_fulfilled, on_rejected];
        let _ = vm.execute_promise_then(&then_args, realm, caller_strict)?;
    }
    Ok(JsValue::Object(result_promise_id))
}

fn is_promise_object_value(vm: &Vm, value: &JsValue) -> bool {
    let JsValue::Object(object_id) = value else {
        return false;
    };
    vm.objects
        .get(object_id)
        .and_then(|object| object.properties.get("__promiseTag"))
        .is_some_and(|tag| matches!(tag, JsValue::Bool(true)))
}

fn promise_resolve_value(
    vm: &mut Vm,
    value: JsValue,
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    if is_promise_object_value(vm, &value) {
        return Ok(value);
    }

    let then = if Vm::is_object_like_value(&value) {
        vm.get_property_from_receiver(value.clone(), "then", realm)?
    } else {
        JsValue::Undefined
    };
    if !Vm::is_callable_value(&then) {
        return vm.create_async_settled_promise(true, value);
    }

    let result_promise_id = vm.create_pending_promise()?;
    let settled_state = Rc::new(RefCell::new(false));

    let resolve_state = Rc::clone(&settled_state);
    let resolve = vm.register_host_callback_function(
        "__qjs_promise_resolve_thenable_resolve__",
        1.0,
        false,
        move |vm, _this_arg, args, _realm, _strict| {
            let resolved_value = args.first().cloned().unwrap_or(JsValue::Undefined);
            let should_settle = {
                let mut settled = resolve_state.borrow_mut();
                if *settled {
                    false
                } else {
                    *settled = true;
                    true
                }
            };
            if should_settle {
                let mut hooks = NoopPromiseJobHostHooks;
                vm.settle_promise_with_hooks(
                    result_promise_id,
                    PromiseSettlement::Fulfilled(resolved_value),
                    &mut hooks,
                )?;
            }
            Ok(JsValue::Undefined)
        },
    );

    let reject_state = Rc::clone(&settled_state);
    let reject = vm.register_host_callback_function(
        "__qjs_promise_resolve_thenable_reject__",
        1.0,
        false,
        move |vm, _this_arg, args, _realm, _strict| {
            let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
            let should_settle = {
                let mut settled = reject_state.borrow_mut();
                if *settled {
                    false
                } else {
                    *settled = true;
                    true
                }
            };
            if should_settle {
                let mut hooks = NoopPromiseJobHostHooks;
                vm.settle_promise_with_hooks(
                    result_promise_id,
                    PromiseSettlement::Rejected(reason),
                    &mut hooks,
                )?;
            }
            Ok(JsValue::Undefined)
        },
    );

    if let Err(err) = vm.execute_callable(
        then,
        Some(value),
        vec![resolve, reject],
        realm,
        caller_strict,
    ) {
        let should_settle = {
            let mut settled = settled_state.borrow_mut();
            if *settled {
                false
            } else {
                *settled = true;
                true
            }
        };
        if should_settle {
            let Some(rejection) = vm.promise_rejection_from_runtime_error(err.clone()) else {
                return Err(err);
            };
            let mut hooks = NoopPromiseJobHostHooks;
            vm.settle_promise_with_hooks(
                result_promise_id,
                PromiseSettlement::Rejected(rejection),
                &mut hooks,
            )?;
        }
    }

    Ok(JsValue::Object(result_promise_id))
}

fn collect_promise_iterable_values(
    vm: &mut Vm,
    items: JsValue,
    realm: &Realm,
    caller_strict: bool,
    type_error_message: &'static str,
) -> Result<Vec<JsValue>, VmError> {
    if matches!(items, JsValue::Null | JsValue::Undefined) {
        return Err(VmError::TypeError(type_error_message));
    }
    let iterator_method = vm.get_property_from_receiver(items.clone(), "Symbol.iterator", realm)?;
    if !matches!(iterator_method, JsValue::Undefined | JsValue::Null) {
        if !Vm::is_callable_value(&iterator_method) {
            return Err(VmError::TypeError(type_error_message));
        }
        let iterator = vm.execute_callable(
            iterator_method,
            Some(items.clone()),
            Vec::new(),
            realm,
            caller_strict,
        )?;
        if !Vm::is_object_like_value(&iterator) {
            return Err(VmError::TypeError(type_error_message));
        }
        let next_method = vm.get_property_from_receiver(iterator.clone(), "next", realm)?;
        if !Vm::is_callable_value(&next_method) {
            return Err(VmError::TypeError(type_error_message));
        }
        let mut values = Vec::new();
        loop {
            let step = vm.execute_callable(
                next_method.clone(),
                Some(iterator.clone()),
                Vec::new(),
                realm,
                caller_strict,
            )?;
            if !Vm::is_object_like_value(&step) {
                return Err(VmError::TypeError(type_error_message));
            }
            let done = vm
                .get_property_from_receiver(step.clone(), "done", realm)
                .map(|value| vm.is_truthy(&value))?;
            if done {
                break;
            }
            values.push(vm.get_property_from_receiver(step, "value", realm)?);
        }
        return Ok(values);
    }

    let source = vm.coerce_object_for_object_builtins(items, type_error_message)?;
    let length_value = vm.get_property_from_receiver(source.clone(), "length", realm)?;
    let raw_length = vm.to_number(&length_value);
    let length = if raw_length.is_finite() && raw_length > 0.0 {
        raw_length.min(u32::MAX as f64).floor() as usize
    } else {
        0usize
    };
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        let value = vm.get_property_from_receiver(source.clone(), &index.to_string(), realm)?;
        values.push(value);
    }
    Ok(values)
}

fn create_promise_all_settled_entry(
    vm: &mut Vm,
    fulfilled: bool,
    value: JsValue,
) -> Result<JsValue, VmError> {
    let entry = vm.create_object_value();
    let object_id = match entry {
        JsValue::Object(object_id) => object_id,
        _ => unreachable!(),
    };
    let object = vm
        .objects
        .get_mut(&object_id)
        .ok_or(VmError::UnknownObject(object_id))?;
    object.properties.insert(
        "status".to_string(),
        JsValue::String(if fulfilled { "fulfilled" } else { "rejected" }.to_string()),
    );
    object
        .property_attributes
        .insert("status".to_string(), PropertyAttributes::default());
    object.properties.insert(
        if fulfilled {
            "value".to_string()
        } else {
            "reason".to_string()
        },
        value,
    );
    object.property_attributes.insert(
        if fulfilled {
            "value".to_string()
        } else {
            "reason".to_string()
        },
        PropertyAttributes::default(),
    );
    Ok(JsValue::Object(object_id))
}
