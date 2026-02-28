#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::JsValue;
use vm::{Vm, perf::HotspotAttribution};

fn run_hotspot_sample(enable_attribution: bool) -> (JsValue, Option<HotspotAttribution>) {
    let script = parse_script(
        "let total = 0; \
         let arr = []; \
         for (let i = 0; i < 4; i = i + 1) { \
           arr[i] = i + 1; \
           total = total + arr[i]; \
         } \
         total;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);
    let mut vm = Vm::default();
    vm.set_hotspot_attribution_enabled(enable_attribution);
    let value = vm.execute(&chunk).expect("script should execute");
    (value, vm.hotspot_attribution_snapshot())
}

#[test]
fn perf_hotspot_attribution_records_opcode_families() {
    let (value_a, snapshot_a) = run_hotspot_sample(true);
    let (value_b, snapshot_b) = run_hotspot_sample(true);

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
fn perf_hotspot_toggle_preserves_semantics() {
    let (disabled_value, disabled_snapshot) = run_hotspot_sample(false);
    let (enabled_value, enabled_snapshot) = run_hotspot_sample(true);

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
