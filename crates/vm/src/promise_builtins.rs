use runtime::{JsValue, NativeFunction, Realm};
use std::cell::RefCell;
use std::rc::Rc;

use crate::{NoopPromiseJobHostHooks, PromiseSettlement, PropertyAttributes, Vm, VmError};

#[derive(Debug, Clone)]
struct PromiseCapability {
    promise: JsValue,
    resolve: JsValue,
    reject: JsValue,
}

#[derive(Debug, Clone)]
struct PromiseAllState {
    resolve: JsValue,
    reject: JsValue,
    values: Vec<Option<JsValue>>,
    remaining: usize,
    settled: bool,
}

#[derive(Debug, Clone)]
struct PromiseAnyState {
    resolve: JsValue,
    reject: JsValue,
    reasons: Vec<Option<JsValue>>,
    remaining: usize,
    settled: bool,
}

#[derive(Debug, Clone)]
struct PromiseAllSettledState {
    resolve: JsValue,
    values: Vec<Option<JsValue>>,
    remaining: usize,
    settled: bool,
}

#[derive(Debug, Clone)]
struct PromiseRaceState {
    resolve: JsValue,
    reject: JsValue,
    settled: bool,
}

pub(super) fn execute_promise_resolve(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let constructor = static_constructor_arg(args);
    if !is_constructor_value(vm, &constructor)? {
        return Err(VmError::TypeError(
            "Promise.resolve requires constructor receiver",
        ));
    }

    let value = static_value_arg(args);
    if matches!(
        constructor,
        JsValue::NativeFunction(NativeFunction::PromiseConstructor)
    ) {
        if is_promise_object_value(vm, &value) {
            return Ok(value);
        }
        return promise_resolve_value(vm, value, realm, caller_strict);
    }

    let capability = create_promise_capability(vm, constructor, realm, caller_strict)?;
    let _ = vm.execute_callable(
        capability.resolve.clone(),
        Some(JsValue::Undefined),
        vec![value],
        realm,
        caller_strict,
    )?;
    Ok(capability.promise)
}

pub(super) fn execute_promise_reject(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let constructor = static_constructor_arg(args);
    if !is_constructor_value(vm, &constructor)? {
        return Err(VmError::TypeError(
            "Promise.reject requires constructor receiver",
        ));
    }

    let reason = static_value_arg(args);
    if matches!(
        constructor,
        JsValue::NativeFunction(NativeFunction::PromiseConstructor)
    ) {
        return vm.create_async_settled_promise(false, reason);
    }

    let capability = create_promise_capability(vm, constructor, realm, caller_strict)?;
    let _ = vm.execute_callable(
        capability.reject.clone(),
        Some(JsValue::Undefined),
        vec![reason],
        realm,
        caller_strict,
    )?;
    Ok(capability.promise)
}

pub(super) fn execute_promise_all(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let constructor = static_constructor_arg(args);
    if !is_constructor_value(vm, &constructor)? {
        return Err(VmError::TypeError(
            "Promise.all requires constructor receiver",
        ));
    }
    let capability = create_promise_capability(vm, constructor.clone(), realm, caller_strict)?;
    let promise_resolve = get_constructor_resolve(vm, &constructor, realm)?;
    let values = collect_promise_iterable_values(
        vm,
        static_value_arg(args),
        realm,
        caller_strict,
        "Promise.all input must be iterable",
    )?;
    if values.is_empty() {
        let aggregate = vm.create_array_from_values(Vec::new())?;
        let _ = vm.execute_callable(
            capability.resolve,
            Some(JsValue::Undefined),
            vec![aggregate],
            realm,
            caller_strict,
        )?;
        return Ok(capability.promise);
    }

    let state = Rc::new(RefCell::new(PromiseAllState {
        resolve: capability.resolve.clone(),
        reject: capability.reject.clone(),
        values: vec![None; values.len()],
        remaining: values.len(),
        settled: false,
    }));

    for (index, value) in values.into_iter().enumerate() {
        let promise = vm.execute_callable(
            promise_resolve.clone(),
            Some(constructor.clone()),
            vec![value],
            realm,
            caller_strict,
        )?;

        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_all_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, realm, caller_strict| {
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
                        Some((state.resolve.clone(), values))
                    } else {
                        None
                    }
                };
                if let Some((resolve, values)) = settle_payload {
                    let aggregate = vm.create_array_from_values(values)?;
                    let _ = vm.execute_callable(
                        resolve,
                        Some(JsValue::Undefined),
                        vec![aggregate],
                        realm,
                        caller_strict,
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
            move |vm, _this_arg, args, realm, caller_strict| {
                let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
                let reject = {
                    let mut state = rejected_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.reject.clone()
                };
                let _ = vm.execute_callable(
                    reject,
                    Some(JsValue::Undefined),
                    vec![reason],
                    realm,
                    caller_strict,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        invoke_then(vm, promise, on_fulfilled, on_rejected, realm, caller_strict)?;
    }
    Ok(capability.promise)
}

pub(super) fn execute_promise_any(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let constructor = static_constructor_arg(args);
    if !is_constructor_value(vm, &constructor)? {
        return Err(VmError::TypeError(
            "Promise.any requires constructor receiver",
        ));
    }
    let capability = create_promise_capability(vm, constructor.clone(), realm, caller_strict)?;
    let promise_resolve = get_constructor_resolve(vm, &constructor, realm)?;
    let values = collect_promise_iterable_values(
        vm,
        static_value_arg(args),
        realm,
        caller_strict,
        "Promise.any input must be iterable",
    )?;
    if values.is_empty() {
        let aggregate = vm.create_array_from_values(Vec::new())?;
        let _ = vm.execute_callable(
            capability.reject,
            Some(JsValue::Undefined),
            vec![aggregate],
            realm,
            caller_strict,
        )?;
        return Ok(capability.promise);
    }

    let state = Rc::new(RefCell::new(PromiseAnyState {
        resolve: capability.resolve.clone(),
        reject: capability.reject.clone(),
        reasons: vec![None; values.len()],
        remaining: values.len(),
        settled: false,
    }));

    for (index, value) in values.into_iter().enumerate() {
        let promise = vm.execute_callable(
            promise_resolve.clone(),
            Some(constructor.clone()),
            vec![value],
            realm,
            caller_strict,
        )?;

        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_any_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, realm, caller_strict| {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                let resolve = {
                    let mut state = fulfilled_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.resolve.clone()
                };
                let _ = vm.execute_callable(
                    resolve,
                    Some(JsValue::Undefined),
                    vec![value],
                    realm,
                    caller_strict,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        let rejected_state = Rc::clone(&state);
        let on_rejected = vm.register_host_callback_function(
            "__qjs_promise_any_rejected__",
            1.0,
            false,
            move |vm, _this_arg, args, realm, caller_strict| {
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
                        Some((state.reject.clone(), reasons))
                    } else {
                        None
                    }
                };
                if let Some((reject, reasons)) = settle_payload {
                    let aggregate = vm.create_array_from_values(reasons)?;
                    let _ = vm.execute_callable(
                        reject,
                        Some(JsValue::Undefined),
                        vec![aggregate],
                        realm,
                        caller_strict,
                    )?;
                }
                Ok(JsValue::Undefined)
            },
        );

        invoke_then(vm, promise, on_fulfilled, on_rejected, realm, caller_strict)?;
    }
    Ok(capability.promise)
}

pub(super) fn execute_promise_race(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let constructor = static_constructor_arg(args);
    if !is_constructor_value(vm, &constructor)? {
        return Err(VmError::TypeError(
            "Promise.race requires constructor receiver",
        ));
    }
    let capability = create_promise_capability(vm, constructor.clone(), realm, caller_strict)?;
    let promise_resolve = get_constructor_resolve(vm, &constructor, realm)?;
    let values = collect_promise_iterable_values(
        vm,
        static_value_arg(args),
        realm,
        caller_strict,
        "Promise.race input must be iterable",
    )?;
    if values.is_empty() {
        return Ok(capability.promise);
    }

    let state = Rc::new(RefCell::new(PromiseRaceState {
        resolve: capability.resolve.clone(),
        reject: capability.reject.clone(),
        settled: false,
    }));
    for value in values {
        let promise = vm.execute_callable(
            promise_resolve.clone(),
            Some(constructor.clone()),
            vec![value],
            realm,
            caller_strict,
        )?;
        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_race_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, realm, caller_strict| {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                let resolve = {
                    let mut state = fulfilled_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.resolve.clone()
                };
                let _ = vm.execute_callable(
                    resolve,
                    Some(JsValue::Undefined),
                    vec![value],
                    realm,
                    caller_strict,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        let rejected_state = Rc::clone(&state);
        let on_rejected = vm.register_host_callback_function(
            "__qjs_promise_race_rejected__",
            1.0,
            false,
            move |vm, _this_arg, args, realm, caller_strict| {
                let reason = args.first().cloned().unwrap_or(JsValue::Undefined);
                let reject = {
                    let mut state = rejected_state.borrow_mut();
                    if state.settled {
                        return Ok(JsValue::Undefined);
                    }
                    state.settled = true;
                    state.reject.clone()
                };
                let _ = vm.execute_callable(
                    reject,
                    Some(JsValue::Undefined),
                    vec![reason],
                    realm,
                    caller_strict,
                )?;
                Ok(JsValue::Undefined)
            },
        );

        invoke_then(vm, promise, on_fulfilled, on_rejected, realm, caller_strict)?;
    }
    Ok(capability.promise)
}

pub(super) fn execute_promise_all_settled(
    vm: &mut Vm,
    args: &[JsValue],
    realm: &Realm,
    caller_strict: bool,
) -> Result<JsValue, VmError> {
    let constructor = static_constructor_arg(args);
    if !is_constructor_value(vm, &constructor)? {
        return Err(VmError::TypeError(
            "Promise.allSettled requires constructor receiver",
        ));
    }
    let capability = create_promise_capability(vm, constructor.clone(), realm, caller_strict)?;
    let promise_resolve = get_constructor_resolve(vm, &constructor, realm)?;
    let values = collect_promise_iterable_values(
        vm,
        static_value_arg(args),
        realm,
        caller_strict,
        "Promise.allSettled input must be iterable",
    )?;
    if values.is_empty() {
        let aggregate = vm.create_array_from_values(Vec::new())?;
        let _ = vm.execute_callable(
            capability.resolve,
            Some(JsValue::Undefined),
            vec![aggregate],
            realm,
            caller_strict,
        )?;
        return Ok(capability.promise);
    }

    let state = Rc::new(RefCell::new(PromiseAllSettledState {
        resolve: capability.resolve.clone(),
        values: vec![None; values.len()],
        remaining: values.len(),
        settled: false,
    }));

    for (index, value) in values.into_iter().enumerate() {
        let promise = vm.execute_callable(
            promise_resolve.clone(),
            Some(constructor.clone()),
            vec![value],
            realm,
            caller_strict,
        )?;
        let fulfilled_state = Rc::clone(&state);
        let on_fulfilled = vm.register_host_callback_function(
            "__qjs_promise_all_settled_fulfilled__",
            1.0,
            false,
            move |vm, _this_arg, args, realm, caller_strict| {
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
                        Some((state.resolve.clone(), values))
                    } else {
                        None
                    }
                };
                if let Some((resolve, values)) = settle_payload {
                    let aggregate = vm.create_array_from_values(values)?;
                    let _ = vm.execute_callable(
                        resolve,
                        Some(JsValue::Undefined),
                        vec![aggregate],
                        realm,
                        caller_strict,
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
            move |vm, _this_arg, args, realm, caller_strict| {
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
                        Some((state.resolve.clone(), values))
                    } else {
                        None
                    }
                };
                if let Some((resolve, values)) = settle_payload {
                    let aggregate = vm.create_array_from_values(values)?;
                    let _ = vm.execute_callable(
                        resolve,
                        Some(JsValue::Undefined),
                        vec![aggregate],
                        realm,
                        caller_strict,
                    )?;
                }
                Ok(JsValue::Undefined)
            },
        );

        invoke_then(vm, promise, on_fulfilled, on_rejected, realm, caller_strict)?;
    }
    Ok(capability.promise)
}

fn static_constructor_arg(args: &[JsValue]) -> JsValue {
    args.first().cloned().unwrap_or(JsValue::Undefined)
}

fn static_value_arg(args: &[JsValue]) -> JsValue {
    args.get(1).cloned().unwrap_or(JsValue::Undefined)
}

fn is_constructor_value(vm: &mut Vm, value: &JsValue) -> Result<bool, VmError> {
    match value {
        JsValue::NativeFunction(native) => Ok(Vm::native_function_is_constructor(*native)),
        JsValue::Function(closure_id) => {
            Ok(!vm.closure_is_arrow(*closure_id)? && !vm.closure_has_no_prototype(*closure_id))
        }
        JsValue::HostFunction(host_id) => Ok(vm.host_function_is_constructable(*host_id)),
        JsValue::Object(object_id) => Ok(vm.is_class_constructor_object(*object_id)),
        _ => Ok(false),
    }
}

fn create_promise_capability(
    vm: &mut Vm,
    constructor: JsValue,
    realm: &Realm,
    caller_strict: bool,
) -> Result<PromiseCapability, VmError> {
    if !is_constructor_value(vm, &constructor)? {
        return Err(VmError::TypeError(
            "Promise capability constructor must be constructor",
        ));
    }

    let resolve_slot = Rc::new(RefCell::new(None::<JsValue>));
    let reject_slot = Rc::new(RefCell::new(None::<JsValue>));
    let resolve_slot_capture = Rc::clone(&resolve_slot);
    let reject_slot_capture = Rc::clone(&reject_slot);
    let executor = vm.register_host_callback_function(
        "__qjs_promise_capability_executor__",
        2.0,
        false,
        move |_vm, _this_arg, args, _realm, _strict| {
            let resolve_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
            let reject_arg = args.get(1).cloned().unwrap_or(JsValue::Undefined);

            if resolve_slot_capture
                .borrow()
                .as_ref()
                .is_some_and(|value| !matches!(value, JsValue::Undefined))
            {
                return Err(VmError::TypeError(
                    "Promise capability executor already initialized",
                ));
            }
            if reject_slot_capture
                .borrow()
                .as_ref()
                .is_some_and(|value| !matches!(value, JsValue::Undefined))
            {
                return Err(VmError::TypeError(
                    "Promise capability executor already initialized",
                ));
            }

            *resolve_slot_capture.borrow_mut() = Some(resolve_arg);
            *reject_slot_capture.borrow_mut() = Some(reject_arg);
            Ok(JsValue::Undefined)
        },
    );

    let promise = vm.execute_construct_value(constructor, vec![executor], realm, caller_strict)?;
    let resolve = resolve_slot.borrow().clone().unwrap_or(JsValue::Undefined);
    let reject = reject_slot.borrow().clone().unwrap_or(JsValue::Undefined);
    if !Vm::is_callable_value(&resolve) || !Vm::is_callable_value(&reject) {
        return Err(VmError::TypeError(
            "Promise capability resolve/reject must be callable",
        ));
    }

    Ok(PromiseCapability {
        promise,
        resolve,
        reject,
    })
}

fn get_constructor_resolve(
    vm: &mut Vm,
    constructor: &JsValue,
    realm: &Realm,
) -> Result<JsValue, VmError> {
    let promise_resolve = vm.get_property_from_receiver(constructor.clone(), "resolve", realm)?;
    if !Vm::is_callable_value(&promise_resolve) {
        return Err(VmError::TypeError(
            "Promise constructor resolve must be callable",
        ));
    }
    Ok(promise_resolve)
}

fn invoke_then(
    vm: &mut Vm,
    promise: JsValue,
    on_fulfilled: JsValue,
    on_rejected: JsValue,
    realm: &Realm,
    caller_strict: bool,
) -> Result<(), VmError> {
    let then = vm.get_property_from_receiver(promise.clone(), "then", realm)?;
    if !Vm::is_callable_value(&then) {
        return Err(VmError::TypeError("Promise.then must be callable"));
    }
    let _ = vm.execute_callable(
        then,
        Some(promise),
        vec![on_fulfilled, on_rejected],
        realm,
        caller_strict,
    )?;
    Ok(())
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

    let promise_id = vm.create_pending_promise()?;
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
                    promise_id,
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
                    promise_id,
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
                promise_id,
                PromiseSettlement::Rejected(rejection),
                &mut hooks,
            )?;
        }
    }

    Ok(JsValue::Object(promise_id))
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
