#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::JsValue;
use vm::{Vm, perf::HotspotAttribution};

fn run_hotspot_sample(
    source: &str,
    enable_attribution: bool,
    packet_d_enabled: bool,
    packet_g_enabled: bool,
    packet_h_enabled: bool,
) -> (JsValue, Option<HotspotAttribution>) {
    let script = parse_script(source).expect("script should parse");
    let chunk = compile_script(&script);
    let mut vm = Vm::default();
    vm.set_hotspot_attribution_enabled(enable_attribution);
    vm.set_packet_c_fast_path_enabled(false);
    vm.set_packet_d_fast_path_enabled(packet_d_enabled);
    vm.set_packet_d_fast_path_metrics_enabled(true);
    vm.set_packet_g_fast_path_enabled(packet_g_enabled);
    vm.set_packet_g_fast_path_metrics_enabled(true);
    vm.set_packet_h_fast_path_enabled(packet_h_enabled);
    vm.set_packet_h_fast_path_metrics_enabled(true);
    let value = vm.execute(&chunk).expect("script should execute");
    (value, vm.hotspot_attribution_snapshot())
}

#[test]
fn perf_hotspot_attribution_records_opcode_families() {
    let source = "let total = 0; \
                  let arr = []; \
                  for (let i = 0; i < 4; i = i + 1) { \
                    arr[i] = i + 1; \
                    total = total + arr[i]; \
                  } \
                  total;";
    let (value_a, snapshot_a) = run_hotspot_sample(source, true, true, false, false);
    let (value_b, snapshot_b) = run_hotspot_sample(source, true, true, false, false);

    assert_eq!(value_a, JsValue::Number(10.0));
    assert_eq!(value_b, JsValue::Number(10.0));

    let snapshot_a = snapshot_a.expect("hotspot attribution should be present");
    let snapshot_b = snapshot_b.expect("hotspot attribution should be present");

    assert!(
        snapshot_a.numeric_ops > 0,
        "numeric opcode counter should increment"
    );
    assert!(
        snapshot_a.identifier_resolution > 0,
        "identifier resolution counter should increment"
    );
    assert!(
        snapshot_a.array_indexed_property_get > 0,
        "array indexed property get counter should increment"
    );
    assert!(
        snapshot_a.array_indexed_property_set > 0,
        "array indexed property set counter should increment"
    );

    assert_eq!(
        snapshot_a, snapshot_b,
        "hotspot counters should be deterministic for identical workloads"
    );
}

#[test]
fn perf_hotspot_attribution_records_packet_guard_taxonomy_deterministically() {
    let source = r#"
let stable = 3;
let total = 0;
for (let i = 0; i < 5; i = i + 1) {
  total = total + stable;
}
for (let j = 0; j < 4; j = j + 1) {
  let token = j;
  total = total + token;
}
with ({ stable: 9 }) {
  total = total + stable;
}
typeof neverDeclared;
total;
"#;

    let (value_a, snapshot_a) = run_hotspot_sample(source, true, true, false, false);
    let (value_b, snapshot_b) = run_hotspot_sample(source, true, true, false, false);
    assert_eq!(value_a, value_b);

    let snapshot_a = snapshot_a.expect("hotspot attribution should be present");
    let snapshot_b = snapshot_b.expect("hotspot attribution should be present");

    assert!(
        snapshot_a.packet_d_slot_guard_hits > 0,
        "stable lexical lookups should produce packet-D slot hits"
    );
    assert!(
        snapshot_a.packet_d_slot_guard_misses > 0,
        "with/scope churn should force packet-D slot misses"
    );
    assert!(
        snapshot_a.packet_d_slot_guard_revalidate_hits
            + snapshot_a.packet_d_slot_guard_revalidate_misses
            == snapshot_b.packet_d_slot_guard_revalidate_hits
                + snapshot_b.packet_d_slot_guard_revalidate_misses,
        "packet-D revalidate buckets must remain deterministic in hotspot snapshots"
    );
    assert!(
        snapshot_a.identifier_resolution_fallback_scans > 0,
        "fallback scan taxonomy should increment when fast guards miss"
    );

    assert_eq!(
        snapshot_a.packet_g_name_guard_hits, 0,
        "packet-G counters must stay visible and deterministic before packet-G path is enabled"
    );
    assert_eq!(
        snapshot_a.packet_g_name_guard_misses, 0,
        "packet-G counters must stay visible and deterministic before packet-G path is enabled"
    );
    assert_eq!(
        snapshot_a.packet_g_name_guard_revalidate_hits, 0,
        "packet-G counters must stay visible and deterministic before packet-G path is enabled"
    );
    assert_eq!(
        snapshot_a.packet_g_name_guard_revalidate_misses, 0,
        "packet-G counters must stay visible and deterministic before packet-G path is enabled"
    );

    assert_eq!(snapshot_a, snapshot_b);
}

#[test]
fn perf_hotspot_attribution_guard_buckets_reflect_workload_shape() {
    let stable_source = r#"
let stable = 2;
let total = 0;
for (let i = 0; i < 12; i = i + 1) {
  total = total + stable;
}
total;
"#;
    let churn_source = r#"
let stable = 2;
let total = 0;
for (let i = 0; i < 6; i = i + 1) {
  total = total + stable;
  {
    let stable = i;
    total = total + stable;
  }
}
with ({ stable: 11 }) {
  total = total + stable;
}
typeof unknownToken;
total;
"#;

    let (_, stable_snapshot) = run_hotspot_sample(stable_source, true, true, false, false);
    let (_, churn_snapshot) = run_hotspot_sample(churn_source, true, true, false, false);
    let stable_snapshot = stable_snapshot.expect("stable run should expose hotspot counters");
    let churn_snapshot = churn_snapshot.expect("churn run should expose hotspot counters");

    assert!(
        stable_snapshot.packet_d_slot_guard_hits > churn_snapshot.packet_d_slot_guard_hits,
        "stable lexical loops should bias packet-D toward hit buckets"
    );
    assert!(
        churn_snapshot.packet_d_slot_guard_misses > stable_snapshot.packet_d_slot_guard_misses,
        "with/scope churn should bias packet-D toward miss buckets"
    );
    assert!(
        churn_snapshot.identifier_resolution_fallback_scans
            > stable_snapshot.identifier_resolution_fallback_scans,
        "scope churn should produce more fallback scan events than stable lexical loops"
    );
}

#[test]
fn perf_hotspot_toggle_preserves_semantics() {
    let source = "let total = 0; \
                  let arr = []; \
                  for (let i = 0; i < 4; i = i + 1) { \
                    arr[i] = i + 1; \
                    total = total + arr[i]; \
                  } \
                  total;";
    let (disabled_value, disabled_snapshot) = run_hotspot_sample(source, false, true, false, false);
    let (enabled_value, enabled_snapshot) = run_hotspot_sample(source, true, true, false, false);

    assert_eq!(disabled_value, enabled_value);
    assert!(
        disabled_snapshot.is_none(),
        "hotspot attribution must stay disabled by default"
    );
    assert!(
        enabled_snapshot.is_some(),
        "enabling hotspot attribution should expose counters"
    );
}

#[test]
fn perf_hotspot_attribution_packet_h_separates_guard_hits_misses_and_fallback_scans() {
    let stable_source = r#"
let stable = 2;
let total = 0;
for (let i = 0; i < 80; i = i + 1) {
  total = total + stable;
  {
    let local = i;
    total = total + local;
  }
}
total;
"#;
    let miss_source = r#"
let stable = 2;
let total = 0;
with ({ stable: 9 }) {
  total = total + stable;
}
typeof neverDeclared;
for (let i = 0; i < 4; i = i + 1) {
  {
    let stable = i;
    total = total + stable;
  }
}
total;
"#;

    let (_, packet_g_stable) = run_hotspot_sample(stable_source, true, true, true, false);
    let (_, packet_h_stable) = run_hotspot_sample(stable_source, true, true, true, true);
    let (_, packet_h_missy) = run_hotspot_sample(miss_source, true, true, true, true);
    let packet_g_stable = packet_g_stable.expect("packet-g snapshot should exist");
    let packet_h_stable = packet_h_stable.expect("packet-h stable snapshot should exist");
    let packet_h_missy = packet_h_missy.expect("packet-h miss snapshot should exist");

    assert!(
        packet_h_stable.packet_h_lexical_slot_guard_hits > 0,
        "stable lexical loops should register packet-h guard hits"
    );
    assert_eq!(
        packet_h_missy.packet_h_lexical_slot_guard_hits, 0,
        "with/unknown/mutation paths should avoid packet-h guard hits"
    );
    assert!(
        packet_h_missy.packet_h_lexical_slot_guard_misses > 0,
        "with/unknown/mutation paths should register packet-h guard misses"
    );
    assert!(
        packet_h_missy.identifier_resolution_fallback_scans > 0,
        "guard misses must keep fallback-scan attribution visible"
    );
    assert!(
        packet_h_stable.identifier_resolution_fallback_scans
            < packet_g_stable.identifier_resolution_fallback_scans,
        "packet-h should reduce fallback scans versus packet-g baseline for stable lexical loops"
    );
}
