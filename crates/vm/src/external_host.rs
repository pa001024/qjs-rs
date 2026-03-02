use bytecode::{Chunk, Opcode};
use runtime::JsValue;
use std::collections::BTreeMap;
use std::fmt;

use crate::{HostFunction, PropertyAttributes, Realm, Vm, VmError};

type ExternalHostCallback =
    dyn FnMut(&mut Vm, Option<JsValue>, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>;

#[derive(Default)]
pub(super) struct ExternalHostCallbacks {
    pub(super) next_id: u64,
    pub(super) entries: BTreeMap<u64, Box<ExternalHostCallback>>,
}

impl fmt::Debug for ExternalHostCallbacks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalHostCallbacks")
            .field("next_id", &self.next_id)
            .field("entry_count", &self.entries.len())
            .finish()
    }
}

impl Vm {
    fn refresh_host_constructor_backlink(&mut self, host_id: u64, prototype: &JsValue) {
        if let JsValue::Object(prototype_id) = prototype
            && let Some(prototype_object) = self.objects.get_mut(prototype_id)
        {
            prototype_object
                .properties
                .insert("constructor".to_string(), JsValue::HostFunction(host_id));
            prototype_object.property_attributes.insert(
                "constructor".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );
        }
    }

    pub fn call_function_value(
        &mut self,
        callee: JsValue,
        this_arg: Option<JsValue>,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        self.execute_callable(callee, this_arg, args, realm, caller_strict)
    }

    pub fn register_host_callback_function<F>(
        &mut self,
        name: &str,
        length: f64,
        constructable: bool,
        callback: F,
    ) -> JsValue
    where
        F: FnMut(&mut Vm, Option<JsValue>, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>
            + 'static,
    {
        let callback_id = self.external_host_callbacks.next_id;
        self.external_host_callbacks.next_id = self
            .external_host_callbacks
            .next_id
            .checked_add(1)
            .expect("external host callback id overflow");
        self.external_host_callbacks
            .entries
            .insert(callback_id, Box::new(callback));
        let function = self.create_host_function_value(HostFunction::ExternalCallback {
            callback_id,
            constructable,
        });
        self.set_builtin_function_name(&function, name);
        self.set_builtin_function_length(&function, length);
        function
    }

    pub fn define_global_host_callback<F>(
        &mut self,
        realm: &Realm,
        name: &str,
        length: f64,
        constructable: bool,
        callback: F,
    ) -> Result<JsValue, VmError>
    where
        F: FnMut(&mut Vm, Option<JsValue>, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>
            + 'static,
    {
        if self.global_object_id.is_none() || self.scopes.is_empty() {
            let bootstrap = Chunk {
                code: vec![Opcode::LoadUndefined, Opcode::Halt],
                functions: Vec::new(),
            };
            let _ = self.execute_in_realm(&bootstrap, realm)?;
        }
        let global_object_id = self
            .global_object_id
            .ok_or(VmError::RuntimeIntegrity("missing global object"))?;
        let function = self.register_host_callback_function(name, length, constructable, callback);
        self.define_global_property_with_attributes(
            global_object_id,
            name,
            function.clone(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        )?;
        Ok(function)
    }

    pub(super) fn execute_external_host_callback(
        &mut self,
        callback_id: u64,
        this_arg: Option<JsValue>,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let mut callback = self
            .external_host_callbacks
            .entries
            .remove(&callback_id)
            .ok_or(VmError::TypeError("HostCallback:Missing"))?;
        let result = callback(self, this_arg, args, realm, caller_strict);
        self.external_host_callbacks
            .entries
            .insert(callback_id, callback);
        result
    }

    pub(super) fn host_function_is_constructable(&self, host_id: u64) -> bool {
        self.host_functions
            .get(&host_id)
            .is_some_and(|host| match host {
                HostFunction::BoundCall { .. } => true,
                HostFunction::ExternalCallback { constructable, .. } => *constructable,
                _ => false,
            })
    }

    pub(super) fn get_or_create_host_function_prototype_property(
        &mut self,
        host_id: u64,
    ) -> Result<JsValue, VmError> {
        if !self.host_functions.contains_key(&host_id) {
            return Err(VmError::UnknownHostFunction(host_id));
        }
        if let Some(existing) = self
            .host_function_objects
            .get(&host_id)
            .and_then(|object| object.properties.get("prototype"))
            .cloned()
        {
            if Self::is_object_like_value(&existing) {
                self.refresh_host_constructor_backlink(host_id, &existing);
                return Ok(existing);
            }
        }

        let prototype = self.create_object_value();
        self.refresh_host_constructor_backlink(host_id, &prototype);

        let object = self.host_function_objects.entry(host_id).or_default();
        object
            .properties
            .insert("prototype".to_string(), prototype.clone());
        object.property_attributes.insert(
            "prototype".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: false,
            },
        );
        Ok(prototype)
    }
}
