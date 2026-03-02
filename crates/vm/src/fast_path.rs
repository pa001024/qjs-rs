#![forbid(unsafe_code)]

use runtime::JsValue;
use rustc_hash::FxHashMap as HashMap;

pub type BindingId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericRelationalOp {
    Lt,
    Le,
    Gt,
    Ge,
}

#[inline(always)]
pub fn fast_number_coercion_candidate(value: &JsValue) -> Option<f64> {
    match value {
        JsValue::Number(number) => Some(*number),
        JsValue::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        _ => None,
    }
}

#[inline(always)]
pub fn try_numeric_binary(
    value_left: &JsValue,
    value_right: &JsValue,
    op: NumericBinaryOp,
) -> Option<f64> {
    let left = fast_number_coercion_candidate(value_left)?;
    let right = fast_number_coercion_candidate(value_right)?;
    let output = match op {
        NumericBinaryOp::Add => left + right,
        NumericBinaryOp::Sub => left - right,
        NumericBinaryOp::Mul => left * right,
        NumericBinaryOp::Div => left / right,
    };
    Some(output)
}

#[inline(always)]
pub fn try_numeric_relational(
    value_left: &JsValue,
    value_right: &JsValue,
    op: NumericRelationalOp,
) -> Option<bool> {
    let left = fast_number_coercion_candidate(value_left)?;
    let right = fast_number_coercion_candidate(value_right)?;
    if left.is_nan() || right.is_nan() {
        return Some(false);
    }
    let output = match op {
        NumericRelationalOp::Lt => left < right,
        NumericRelationalOp::Le => left <= right,
        NumericRelationalOp::Gt => left > right,
        NumericRelationalOp::Ge => left >= right,
    };
    Some(output)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BindingCacheEntry {
    pub scope_index: usize,
    pub binding_id: BindingId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketAFastPathCounters {
    pub numeric_guard_hits: u64,
    pub numeric_guard_misses: u64,
    pub binding_guard_hits: u64,
    pub binding_guard_misses: u64,
}

#[derive(Debug, Default)]
pub struct PacketAFastPathState {
    binding_cache: HashMap<String, BindingCacheEntry>,
    counters: PacketAFastPathCounters,
}

impl PacketAFastPathState {
    pub fn reset(&mut self) {
        self.binding_cache.clear();
        self.counters = PacketAFastPathCounters::default();
    }

    pub fn clear_binding_cache(&mut self) {
        self.binding_cache.clear();
    }

    pub fn remove_binding_cache_entry(&mut self, name: &str) {
        self.binding_cache.remove(name);
    }

    pub fn binding_cache_entry(&self, name: &str) -> Option<BindingCacheEntry> {
        self.binding_cache.get(name).copied()
    }

    pub fn remember_binding_cache_entry(
        &mut self,
        name: &str,
        scope_index: usize,
        binding_id: BindingId,
    ) {
        self.binding_cache.insert(
            name.to_string(),
            BindingCacheEntry {
                scope_index,
                binding_id,
            },
        );
    }

    pub fn record_numeric_guard_hit(&mut self) {
        self.counters.numeric_guard_hits = self.counters.numeric_guard_hits.wrapping_add(1);
    }

    pub fn record_numeric_guard_miss(&mut self) {
        self.counters.numeric_guard_misses = self.counters.numeric_guard_misses.wrapping_add(1);
    }

    pub fn record_binding_guard_hit(&mut self) {
        self.counters.binding_guard_hits = self.counters.binding_guard_hits.wrapping_add(1);
    }

    pub fn record_binding_guard_miss(&mut self) {
        self.counters.binding_guard_misses = self.counters.binding_guard_misses.wrapping_add(1);
    }

    pub fn counters(&self) -> PacketAFastPathCounters {
        self.counters
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketBFastPathCounters {
    pub dense_array_get_guard_hits: u64,
    pub dense_array_get_guard_misses: u64,
    pub dense_array_set_guard_hits: u64,
    pub dense_array_set_guard_misses: u64,
}

#[derive(Debug, Default)]
pub struct PacketBFastPathState {
    counters: PacketBFastPathCounters,
}

impl PacketBFastPathState {
    pub fn reset(&mut self) {
        self.counters = PacketBFastPathCounters::default();
    }

    pub fn record_dense_array_get_guard_hit(&mut self) {
        self.counters.dense_array_get_guard_hits =
            self.counters.dense_array_get_guard_hits.wrapping_add(1);
    }

    pub fn record_dense_array_get_guard_miss(&mut self) {
        self.counters.dense_array_get_guard_misses =
            self.counters.dense_array_get_guard_misses.wrapping_add(1);
    }

    pub fn record_dense_array_set_guard_hit(&mut self) {
        self.counters.dense_array_set_guard_hits =
            self.counters.dense_array_set_guard_hits.wrapping_add(1);
    }

    pub fn record_dense_array_set_guard_miss(&mut self) {
        self.counters.dense_array_set_guard_misses =
            self.counters.dense_array_set_guard_misses.wrapping_add(1);
    }

    pub fn counters(&self) -> PacketBFastPathCounters {
        self.counters
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketCFastPathCounters {
    pub identifier_guard_hits: u64,
    pub identifier_guard_misses: u64,
    pub global_guard_hits: u64,
    pub global_guard_misses: u64,
}

#[derive(Debug, Default)]
pub struct PacketCFastPathState {
    binding_cache: HashMap<String, BindingCacheEntry>,
    counters: PacketCFastPathCounters,
}

impl PacketCFastPathState {
    pub fn reset(&mut self) {
        self.binding_cache.clear();
        self.counters = PacketCFastPathCounters::default();
    }

    pub fn clear_binding_cache(&mut self) {
        self.binding_cache.clear();
    }

    pub fn remove_binding_cache_entry(&mut self, name: &str) {
        self.binding_cache.remove(name);
    }

    pub fn binding_cache_entry(&self, name: &str) -> Option<BindingCacheEntry> {
        self.binding_cache.get(name).copied()
    }

    pub fn remember_binding_cache_entry(
        &mut self,
        name: &str,
        scope_index: usize,
        binding_id: BindingId,
    ) {
        self.binding_cache.insert(
            name.to_string(),
            BindingCacheEntry {
                scope_index,
                binding_id,
            },
        );
    }

    pub fn record_identifier_guard_hit(&mut self) {
        self.counters.identifier_guard_hits = self.counters.identifier_guard_hits.wrapping_add(1);
    }

    pub fn record_identifier_guard_miss(&mut self) {
        self.counters.identifier_guard_misses =
            self.counters.identifier_guard_misses.wrapping_add(1);
    }

    pub fn record_global_guard_hit(&mut self) {
        self.counters.global_guard_hits = self.counters.global_guard_hits.wrapping_add(1);
    }

    pub fn record_global_guard_miss(&mut self) {
        self.counters.global_guard_misses = self.counters.global_guard_misses.wrapping_add(1);
    }

    pub fn counters(&self) -> PacketCFastPathCounters {
        self.counters
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketDSlotCacheEntry {
    pub scope_index: usize,
    pub binding_id: BindingId,
    pub scope_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketDFastPathCounters {
    pub slot_guard_hits: u64,
    pub slot_guard_misses: u64,
    pub global_guard_hits: u64,
    pub global_guard_misses: u64,
    pub identifier_call_direct_hits: u64,
    pub identifier_call_direct_misses: u64,
}

#[derive(Debug, Default)]
pub struct PacketDFastPathState {
    slot_cache: Vec<Option<PacketDSlotCacheEntry>>,
    counters: PacketDFastPathCounters,
}

impl PacketDFastPathState {
    pub fn reset(&mut self) {
        self.slot_cache.clear();
        self.counters = PacketDFastPathCounters::default();
    }

    pub fn remove_slot_cache_entry(&mut self, slot: u32) {
        let index = slot as usize;
        if let Some(entry) = self.slot_cache.get_mut(index) {
            *entry = None;
        }
    }

    pub fn slot_cache_entry(&self, slot: u32) -> Option<PacketDSlotCacheEntry> {
        self.slot_cache.get(slot as usize).copied().flatten()
    }

    pub fn remember_slot_cache_entry(
        &mut self,
        slot: u32,
        scope_index: usize,
        binding_id: BindingId,
        scope_generation: u64,
    ) {
        let index = slot as usize;
        if index >= self.slot_cache.len() {
            self.slot_cache.resize(index + 1, None);
        }
        self.slot_cache[index] = Some(PacketDSlotCacheEntry {
            scope_index,
            binding_id,
            scope_generation,
        });
    }

    pub fn record_slot_guard_hit(&mut self) {
        self.counters.slot_guard_hits = self.counters.slot_guard_hits.wrapping_add(1);
    }

    pub fn record_slot_guard_miss(&mut self) {
        self.counters.slot_guard_misses = self.counters.slot_guard_misses.wrapping_add(1);
    }

    pub fn record_global_guard_hit(&mut self) {
        self.counters.global_guard_hits = self.counters.global_guard_hits.wrapping_add(1);
    }

    pub fn record_global_guard_miss(&mut self) {
        self.counters.global_guard_misses = self.counters.global_guard_misses.wrapping_add(1);
    }

    pub fn record_identifier_call_direct_hit(&mut self) {
        self.counters.identifier_call_direct_hits =
            self.counters.identifier_call_direct_hits.wrapping_add(1);
    }

    pub fn record_identifier_call_direct_miss(&mut self) {
        self.counters.identifier_call_direct_misses =
            self.counters.identifier_call_direct_misses.wrapping_add(1);
    }

    pub fn counters(&self) -> PacketDFastPathCounters {
        self.counters
    }
}
