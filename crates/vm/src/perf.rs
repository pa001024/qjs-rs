#![forbid(unsafe_code)]

const ENV_VM_HOTSPOT_ATTRIBUTION: &str = "QJS_RS_VM_HOTSPOT_ATTRIBUTION";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HotspotAttribution {
    pub numeric_ops: u64,
    pub identifier_resolution: u64,
    pub array_indexed_property_get: u64,
    pub array_indexed_property_set: u64,
}

impl HotspotAttribution {
    pub fn merge(&mut self, other: HotspotAttribution) {
        self.numeric_ops = self.numeric_ops.saturating_add(other.numeric_ops);
        self.identifier_resolution = self
            .identifier_resolution
            .saturating_add(other.identifier_resolution);
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

    pub fn record_numeric_op(&mut self) {
        if !self.enabled {
            return;
        }
        self.counters.numeric_ops = self.counters.numeric_ops.saturating_add(1);
    }

    pub fn record_identifier_resolution(&mut self) {
        if !self.enabled {
            return;
        }
        self.counters.identifier_resolution = self.counters.identifier_resolution.saturating_add(1);
    }

    pub fn record_array_indexed_property_get(&mut self) {
        if !self.enabled {
            return;
        }
        self.counters.array_indexed_property_get =
            self.counters.array_indexed_property_get.saturating_add(1);
    }

    pub fn record_array_indexed_property_set(&mut self) {
        if !self.enabled {
            return;
        }
        self.counters.array_indexed_property_set =
            self.counters.array_indexed_property_set.saturating_add(1);
    }
}

pub fn hotspot_attribution_enabled_from_env() -> bool {
    std::env::var(ENV_VM_HOTSPOT_ATTRIBUTION)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}
