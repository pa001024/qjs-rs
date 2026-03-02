use runtime::JsValue;
use rustc_hash::FxHashMap as HashMap;
use std::any::Any;
use std::fmt;

use crate::{ObjectId, TYPE_ERROR_OPAQUE_UNSUPPORTED_VALUE, Vm, VmError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum OpaqueBindingTarget {
    Object(ObjectId),
    Closure(u64),
    HostFunction(u64),
}

#[derive(Default)]
pub(super) struct OpaqueBindings {
    pub(super) entries: HashMap<OpaqueBindingTarget, Box<dyn Any>>,
}

impl fmt::Debug for OpaqueBindings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpaqueBindings")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}

impl Vm {
    pub fn bind_opaque_data<T: Any>(&mut self, value: &JsValue, data: T) -> Result<(), VmError> {
        let Some(target) = Self::opaque_binding_target(value) else {
            return Err(VmError::TypeError(TYPE_ERROR_OPAQUE_UNSUPPORTED_VALUE));
        };
        self.opaque_bindings.entries.insert(target, Box::new(data));
        Ok(())
    }

    pub fn opaque_data<T: Any>(&self, value: &JsValue) -> Option<&T> {
        let target = Self::opaque_binding_target(value)?;
        self.opaque_bindings
            .entries
            .get(&target)
            .and_then(|entry| entry.downcast_ref::<T>())
    }

    pub fn opaque_data_mut<T: Any>(&mut self, value: &JsValue) -> Option<&mut T> {
        let target = Self::opaque_binding_target(value)?;
        self.opaque_bindings
            .entries
            .get_mut(&target)
            .and_then(|entry| entry.downcast_mut::<T>())
    }

    pub fn take_opaque_data<T: Any>(&mut self, value: &JsValue) -> Option<T> {
        let target = Self::opaque_binding_target(value)?;
        self.opaque_bindings
            .entries
            .remove(&target)
            .and_then(|entry| entry.downcast::<T>().ok().map(|boxed| *boxed))
    }

    pub fn clear_opaque_data(&mut self, value: &JsValue) -> bool {
        let Some(target) = Self::opaque_binding_target(value) else {
            return false;
        };
        self.opaque_bindings.entries.remove(&target).is_some()
    }

    pub(super) fn clear_opaque_data_for_object(&mut self, object_id: ObjectId) {
        self.opaque_bindings
            .entries
            .remove(&OpaqueBindingTarget::Object(object_id));
    }

    pub(super) fn opaque_binding_target(value: &JsValue) -> Option<OpaqueBindingTarget> {
        match value {
            JsValue::Object(object_id) => Some(OpaqueBindingTarget::Object(*object_id)),
            JsValue::Function(closure_id) => Some(OpaqueBindingTarget::Closure(*closure_id)),
            JsValue::HostFunction(host_id) => Some(OpaqueBindingTarget::HostFunction(*host_id)),
            _ => None,
        }
    }
}
