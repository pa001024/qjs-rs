#![forbid(unsafe_code)]

const ENV_VM_HOTSPOT_ATTRIBUTION: &str = "QJS_RS_VM_HOTSPOT_ATTRIBUTION";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HotspotAttribution {
    pub numeric_ops: u64,
    pub identifier_resolution: u64,
    pub identifier_resolution_fallback_scans: u64,
    pub packet_d_slot_guard_hits: u64,
    pub packet_d_slot_guard_misses: u64,
    pub packet_d_slot_guard_revalidate_hits: u64,
    pub packet_d_slot_guard_revalidate_misses: u64,
    pub packet_g_name_guard_hits: u64,
    pub packet_g_name_guard_misses: u64,
    pub packet_g_name_guard_revalidate_hits: u64,
    pub packet_g_name_guard_revalidate_misses: u64,
    pub array_indexed_property_get: u64,
    pub array_indexed_property_set: u64,
}

impl HotspotAttribution {
    pub fn merge(&mut self, other: HotspotAttribution) {
        self.numeric_ops = self.numeric_ops.saturating_add(other.numeric_ops);
        self.identifier_resolution = self
            .identifier_resolution
            .saturating_add(other.identifier_resolution);
        self.identifier_resolution_fallback_scans = self
            .identifier_resolution_fallback_scans
            .saturating_add(other.identifier_resolution_fallback_scans);
        self.packet_d_slot_guard_hits = self
            .packet_d_slot_guard_hits
            .saturating_add(other.packet_d_slot_guard_hits);
        self.packet_d_slot_guard_misses = self
            .packet_d_slot_guard_misses
            .saturating_add(other.packet_d_slot_guard_misses);
        self.packet_d_slot_guard_revalidate_hits = self
            .packet_d_slot_guard_revalidate_hits
            .saturating_add(other.packet_d_slot_guard_revalidate_hits);
        self.packet_d_slot_guard_revalidate_misses = self
            .packet_d_slot_guard_revalidate_misses
            .saturating_add(other.packet_d_slot_guard_revalidate_misses);
        self.packet_g_name_guard_hits = self
            .packet_g_name_guard_hits
            .saturating_add(other.packet_g_name_guard_hits);
        self.packet_g_name_guard_misses = self
            .packet_g_name_guard_misses
            .saturating_add(other.packet_g_name_guard_misses);
        self.packet_g_name_guard_revalidate_hits = self
            .packet_g_name_guard_revalidate_hits
            .saturating_add(other.packet_g_name_guard_revalidate_hits);
        self.packet_g_name_guard_revalidate_misses = self
            .packet_g_name_guard_revalidate_misses
            .saturating_add(other.packet_g_name_guard_revalidate_misses);
        self.array_indexed_property_get = self
            .array_indexed_property_get
            .saturating_add(other.array_indexed_property_get);
        self.array_indexed_property_set = self
            .array_indexed_property_set
            .saturating_add(other.array_indexed_property_set);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HotspotAttributionState {
    enabled: bool,
    counters: HotspotAttribution,
}

impl HotspotAttributionState {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.counters = HotspotAttribution::default();
    }

    pub fn snapshot(&self) -> Option<HotspotAttribution> {
        self.enabled.then_some(self.counters)
    }

    #[inline(always)]
    pub fn record_numeric_op_unchecked(&mut self) {
        self.counters.numeric_ops = self.counters.numeric_ops.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_numeric_op(&mut self) {
        if !self.enabled {
            return;
        }
        self.record_numeric_op_unchecked();
    }

    #[inline(always)]
    pub fn record_identifier_resolution_unchecked(&mut self) {
        self.counters.identifier_resolution = self.counters.identifier_resolution.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_identifier_resolution_fallback_scan_unchecked(&mut self) {
        self.counters.identifier_resolution_fallback_scans = self
            .counters
            .identifier_resolution_fallback_scans
            .wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_d_slot_guard_hit_unchecked(&mut self) {
        self.counters.packet_d_slot_guard_hits =
            self.counters.packet_d_slot_guard_hits.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_d_slot_guard_miss_unchecked(&mut self) {
        self.counters.packet_d_slot_guard_misses =
            self.counters.packet_d_slot_guard_misses.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_d_slot_guard_revalidate_hit_unchecked(&mut self) {
        self.counters.packet_d_slot_guard_revalidate_hits = self
            .counters
            .packet_d_slot_guard_revalidate_hits
            .wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_d_slot_guard_revalidate_miss_unchecked(&mut self) {
        self.counters.packet_d_slot_guard_revalidate_misses = self
            .counters
            .packet_d_slot_guard_revalidate_misses
            .wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_g_name_guard_hit_unchecked(&mut self) {
        self.counters.packet_g_name_guard_hits =
            self.counters.packet_g_name_guard_hits.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_g_name_guard_miss_unchecked(&mut self) {
        self.counters.packet_g_name_guard_misses =
            self.counters.packet_g_name_guard_misses.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_g_name_guard_revalidate_hit_unchecked(&mut self) {
        self.counters.packet_g_name_guard_revalidate_hits = self
            .counters
            .packet_g_name_guard_revalidate_hits
            .wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_packet_g_name_guard_revalidate_miss_unchecked(&mut self) {
        self.counters.packet_g_name_guard_revalidate_misses = self
            .counters
            .packet_g_name_guard_revalidate_misses
            .wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_identifier_resolution(&mut self) {
        if !self.enabled {
            return;
        }
        self.record_identifier_resolution_unchecked();
    }

    #[inline(always)]
    pub fn record_array_indexed_property_get_unchecked(&mut self) {
        self.counters.array_indexed_property_get =
            self.counters.array_indexed_property_get.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_array_indexed_property_get(&mut self) {
        if !self.enabled {
            return;
        }
        self.record_array_indexed_property_get_unchecked();
    }

    #[inline(always)]
    pub fn record_array_indexed_property_set_unchecked(&mut self) {
        self.counters.array_indexed_property_set =
            self.counters.array_indexed_property_set.wrapping_add(1);
    }

    #[inline(always)]
    pub fn record_array_indexed_property_set(&mut self) {
        if !self.enabled {
            return;
        }
        self.record_array_indexed_property_set_unchecked();
    }
}

pub fn hotspot_attribution_enabled_from_env() -> bool {
    std::env::var(ENV_VM_HOTSPOT_ATTRIBUTION)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}
