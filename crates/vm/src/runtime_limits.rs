use runtime::JsValue;
use std::mem::size_of;

use crate::opaque_bindings::OpaqueBindingTarget;
use crate::{
    Binding, BindingId, Closure, HostFunction, JsObject, ModuleRecord, PropertyAttributes, Vm,
    VmError, VmInterruptHandler,
};

impl Vm {
    pub fn set_memory_limit_bytes(&mut self, limit: Option<usize>) {
        self.memory_limit_bytes = limit;
        self.memory_check_tick = 0;
    }

    pub fn memory_limit_bytes(&self) -> Option<usize> {
        self.memory_limit_bytes
    }

    pub fn set_memory_check_interval(&mut self, interval: usize) {
        self.memory_check_interval = interval.max(1);
        self.memory_check_tick = 0;
    }

    pub fn memory_check_interval(&self) -> usize {
        self.memory_check_interval
    }

    pub fn set_max_stack_size(&mut self, max_stack_size: Option<usize>) {
        self.max_stack_size = max_stack_size;
    }

    pub fn max_stack_size(&self) -> Option<usize> {
        self.max_stack_size
    }

    pub fn set_interrupt_poll_interval(&mut self, interval: usize) {
        self.interrupt_state.poll_interval = interval.max(1);
        self.interrupt_state.tick = 0;
    }

    pub fn interrupt_poll_interval(&self) -> usize {
        self.interrupt_state.poll_interval
    }

    pub fn set_interrupt_handler_boxed(&mut self, handler: Option<Box<VmInterruptHandler>>) {
        self.interrupt_state.handler = handler;
        self.interrupt_state.tick = 0;
    }

    pub fn set_interrupt_handler<F>(&mut self, handler: F)
    where
        F: FnMut() -> bool + 'static,
    {
        self.set_interrupt_handler_boxed(Some(Box::new(handler)));
    }

    pub fn clear_interrupt_handler(&mut self) {
        self.set_interrupt_handler_boxed(None);
    }

    pub fn estimated_memory_usage_bytes(&self) -> usize {
        self.estimated_memory_usage_bytes_internal()
    }

    pub(super) fn enforce_runtime_limits(&mut self) -> Result<(), VmError> {
        self.poll_interrupt_handler()?;
        self.enforce_stack_limit()?;
        self.maybe_check_memory_limit()?;
        Ok(())
    }

    fn poll_interrupt_handler(&mut self) -> Result<(), VmError> {
        if self.interrupt_state.handler.is_none() {
            return Ok(());
        }
        self.interrupt_state.tick = self.interrupt_state.tick.saturating_add(1);
        if self.interrupt_state.tick % self.interrupt_state.poll_interval.max(1) != 0 {
            return Ok(());
        }
        if let Some(handler) = self.interrupt_state.handler.as_mut()
            && handler()
        {
            return Err(VmError::Interrupted);
        }
        Ok(())
    }

    fn enforce_stack_limit(&self) -> Result<(), VmError> {
        if let Some(max_stack_size) = self.max_stack_size
            && self.stack.len() > max_stack_size
        {
            return Err(VmError::StackLimitExceeded {
                max_stack_size,
                current_stack_size: self.stack.len(),
            });
        }
        Ok(())
    }

    fn maybe_check_memory_limit(&mut self) -> Result<(), VmError> {
        if self.memory_limit_bytes.is_none() {
            return Ok(());
        }
        self.memory_check_tick = self.memory_check_tick.saturating_add(1);
        if self.memory_check_tick % self.memory_check_interval.max(1) != 0 {
            return Ok(());
        }
        self.enforce_memory_limit()
    }

    fn enforce_memory_limit(&self) -> Result<(), VmError> {
        let Some(limit_bytes) = self.memory_limit_bytes else {
            return Ok(());
        };
        let estimated_bytes = self.estimated_memory_usage_bytes_internal();
        if estimated_bytes > limit_bytes {
            return Err(VmError::MemoryLimitExceeded {
                limit_bytes,
                estimated_bytes,
            });
        }
        Ok(())
    }

    fn estimated_memory_usage_bytes_internal(&self) -> usize {
        let mut total = 0usize;

        total = total.saturating_add(size_of::<Self>());
        total = total.saturating_add(self.stack.capacity().saturating_mul(size_of::<JsValue>()));
        total = total.saturating_add(
            self.bindings
                .capacity()
                .saturating_mul(size_of::<Option<Binding>>()),
        );
        total = total.saturating_add(
            self.global_property_sync_bindings
                .capacity()
                .saturating_mul(size_of::<bool>()),
        );

        for scope_ref in &self.scopes {
            let scope = scope_ref.borrow();
            total = total.saturating_add(scope.iter().fold(0usize, |acc, (name, _)| {
                acc.saturating_add(size_of::<String>() + name.len() + size_of::<BindingId>())
            }));
        }

        for binding in self.bindings.iter().flatten() {
            total = total.saturating_add(Self::estimate_js_value_bytes(&binding.value));
            total = total.saturating_add(size_of::<Binding>());
        }

        for object in self.objects.values() {
            total = total.saturating_add(size_of::<JsObject>());
            total = total.saturating_add(object.properties.iter().fold(
                0usize,
                |acc, (name, value)| {
                    acc.saturating_add(size_of::<String>() + name.len())
                        .saturating_add(Self::estimate_js_value_bytes(value))
                },
            ));
            total =
                total.saturating_add(object.getters.iter().fold(0usize, |acc, (name, value)| {
                    acc.saturating_add(size_of::<String>() + name.len())
                        .saturating_add(Self::estimate_js_value_bytes(value))
                }));
            total =
                total.saturating_add(object.setters.iter().fold(0usize, |acc, (name, value)| {
                    acc.saturating_add(size_of::<String>() + name.len())
                        .saturating_add(Self::estimate_js_value_bytes(value))
                }));
            total = total.saturating_add(object.property_attributes.iter().fold(
                0usize,
                |acc, (name, _)| {
                    acc.saturating_add(size_of::<String>() + name.len())
                        .saturating_add(size_of::<PropertyAttributes>())
                },
            ));
            if let Some(dense_elements) = object.dense_elements.as_ref() {
                total = total.saturating_add(
                    dense_elements
                        .capacity()
                        .saturating_mul(size_of::<Option<JsValue>>()),
                );
                total = total.saturating_add(dense_elements.iter().fold(0usize, |acc, entry| {
                    acc.saturating_add(
                        entry
                            .as_ref()
                            .map(Self::estimate_js_value_bytes)
                            .unwrap_or(0),
                    )
                }));
            }
        }

        total = total.saturating_add(
            self.objects
                .len()
                .saturating_mul(size_of::<(crate::ObjectId, JsObject)>()),
        );
        total = total.saturating_add(
            self.closures
                .len()
                .saturating_mul(size_of::<(u64, Closure)>()),
        );
        total = total.saturating_add(
            self.host_functions
                .len()
                .saturating_mul(size_of::<(u64, HostFunction)>()),
        );
        total = total.saturating_add(
            self.host_function_objects
                .len()
                .saturating_mul(size_of::<(u64, JsObject)>()),
        );
        total = total.saturating_add(
            self.external_host_callbacks
                .entries
                .len()
                .saturating_mul(size_of::<(u64, usize)>()),
        );
        total = total.saturating_add(self.host_pins.values().fold(0usize, |acc, value| {
            acc.saturating_add(Self::estimate_js_value_bytes(value))
        }));
        total = total.saturating_add(
            self.module_records
                .len()
                .saturating_mul(size_of::<(String, ModuleRecord)>()),
        );
        total = total.saturating_add(
            self.module_cache_root_candidates
                .values()
                .fold(0usize, |acc, value| {
                    acc.saturating_add(Self::estimate_js_value_bytes(value))
                }),
        );
        total = total.saturating_add(
            self.pending_job_root_candidates
                .values()
                .fold(0usize, |acc, value| {
                    acc.saturating_add(Self::estimate_js_value_bytes(value))
                }),
        );
        total = total.saturating_add(
            self.opaque_bindings
                .entries
                .len()
                .saturating_mul(size_of::<(OpaqueBindingTarget, usize)>()),
        );

        total
    }

    fn estimate_js_value_bytes(value: &JsValue) -> usize {
        let mut size = size_of::<JsValue>();
        if let JsValue::String(text) = value {
            size = size.saturating_add(size_of::<String>());
            size = size.saturating_add(text.len());
        }
        size
    }
}
