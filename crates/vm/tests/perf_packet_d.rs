#![forbid(unsafe_code)]

use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use vm::{PacketDFastPathCounters, PacketGFastPathCounters, Vm, perf::HotspotAttribution};

fn run_script(
    source: &str,
    packet_d_enabled: bool,
    packet_g_enabled: bool,
) -> (
    JsValue,
    PacketDFastPathCounters,
    PacketGFastPathCounters,
    Option<HotspotAttribution>,
) {
    let script = parse_script(source).expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    install_baseline(&mut realm);

    let mut vm = Vm::default();
    vm.set_hotspot_attribution_enabled(true);
    vm.reset_hotspot_attribution();
    vm.set_packet_c_fast_path_enabled(false);
    vm.set_packet_d_fast_path_enabled(packet_d_enabled);
    vm.set_packet_d_fast_path_metrics_enabled(true);
    vm.set_packet_g_fast_path_enabled(packet_g_enabled);
    vm.set_packet_g_fast_path_metrics_enabled(true);

    let value = vm
        .execute_in_realm(&chunk, &realm)
        .expect("script should execute");
    (
        value,
        vm.packet_d_fast_path_counters(),
        vm.packet_g_fast_path_counters(),
        vm.hotspot_attribution_snapshot(),
    )
}

fn assert_packet_d_parity(source: &str) -> (JsValue, PacketDFastPathCounters, HotspotAttribution) {
    let (slow_value, slow_d_counters, slow_g_counters, _slow_hotspot) =
        run_script(source, false, false);
    let (fast_value, fast_d_counters, fast_g_counters, fast_hotspot) =
        run_script(source, true, false);

    assert_eq!(fast_value, slow_value);
    assert_eq!(
        slow_d_counters,
        PacketDFastPathCounters::default(),
        "packet-D counters must stay zero when optimization is disabled"
    );
    assert_eq!(
        slow_g_counters,
        PacketGFastPathCounters::default(),
        "packet-G counters must stay zero when optimization is disabled"
    );
    assert_eq!(
        fast_g_counters,
        PacketGFastPathCounters::default(),
        "packet-D parity runs should keep packet-G disabled"
    );

    (
        fast_value,
        fast_d_counters,
        fast_hotspot.expect("hotspot attribution should be present"),
    )
}

fn assert_packet_g_toggle_parity(
    source: &str,
    packet_d_enabled: bool,
) -> (JsValue, PacketGFastPathCounters, HotspotAttribution) {
    let (slow_value, _slow_d_counters, slow_g_counters, _slow_hotspot) =
        run_script(source, packet_d_enabled, false);
    let (fast_value, _fast_d_counters, fast_g_counters, fast_hotspot) =
        run_script(source, packet_d_enabled, true);
    assert_eq!(fast_value, slow_value);
    assert_eq!(
        slow_g_counters,
        PacketGFastPathCounters::default(),
        "packet-G counters must stay zero when packet-g optimization is disabled"
    );
    (
        fast_value,
        fast_g_counters,
        fast_hotspot.expect("hotspot attribution should be present"),
    )
}

#[test]
fn packet_d_identifier_slot_fast_path_guarding() {
    let source = r#"
globalFast = 5;
let token = 2;
let sum = 0;
for (let i = 0; i < 12; i = i + 1) {
  sum = sum + token;
  sum = sum + globalFast;
}
{
  let token = 9;
  sum = sum + token;
}
sum = sum + token;
let scopeObj = { token: 40 };
with (scopeObj) {
  sum = sum + token;
  token = token + 1;
}
Object.defineProperty(globalThis, "globalFast", {
  get: function() { return 11; },
  set: function(value) { this.__packetDWrite = value; },
  configurable: true
});
sum = sum + globalFast;
globalThis.globalFast = 3;
delete globalThis.globalFast;
globalFast = 4;
sum + globalFast + scopeObj.token + (globalThis.__packetDWrite === 3 ? 1 : 0);
"#;

    let (value, counters, hotspot) = assert_packet_d_parity(source);
    assert_eq!(value, JsValue::Number(192.0));
    assert!(
        counters.slot_guard_hits > 0,
        "stable lexical loads should trigger packet-D slot cache hits"
    );
    assert!(
        counters.slot_guard_misses > 0,
        "with scopes and scope transitions must trigger packet-D slot misses"
    );
    assert!(
        counters.global_guard_hits > 0,
        "global own-data lookups should trigger packet-D global hits"
    );
    assert!(
        counters.global_guard_misses > 0,
        "accessor/prototype-sensitive global lookups must trigger packet-D misses"
    );
    assert!(
        hotspot.identifier_resolution > 0,
        "packet-D should preserve identifier-resolution hotspot attribution"
    );
}

#[test]
fn perf_packet_d_typeof_unknown_identifier_parity() {
    let source = r#"typeof neverDeclared;"#;

    let (value, counters, hotspot) = assert_packet_d_parity(source);
    assert_eq!(value, JsValue::String("undefined".to_string()));
    assert!(
        counters.slot_guard_misses > 0,
        "unknown identifiers must route through packet-D miss path"
    );
    let _ = hotspot;
}

#[test]
fn perf_packet_d_with_scope_lookup_parity() {
    let source = r#"
var outer = 2;
var obj = { outer: 6 };
var sum = 0;
with (obj) {
  sum = sum + outer;
  outer = outer + 1;
}
sum = sum + outer + obj.outer;
sum;
"#;

    let (value, counters, _) = assert_packet_d_parity(source);
    assert_eq!(value, JsValue::Number(15.0));
    assert!(
        counters.slot_guard_misses > 0,
        "with lookups must bypass packet-D slot cache"
    );
}

#[test]
fn perf_packet_d_identifier_call_direct_dispatch_guarding() {
    let source = r#"
function step(value) {
  return value + 1;
}
globalThis.packetDGlobalStep = function(value) {
  return value + 10;
};
let total = 0;
for (let i = 0; i < 6; i = i + 1) {
  total = step(total);
}
total = packetDGlobalStep(total);
delete globalThis.packetDGlobalStep;
total;
"#;

    let (value, counters, hotspot) = assert_packet_d_parity(source);
    assert_eq!(value, JsValue::Number(16.0));
    assert!(
        counters.identifier_call_direct_hits > 0,
        "stable lexical call identifiers should hit packet-D direct call dispatch"
    );
    assert!(
        counters.identifier_call_direct_misses > 0,
        "global-property call identifiers must miss packet-D direct call dispatch and fallback"
    );
    assert!(
        hotspot.identifier_resolution > 0,
        "call identifier workloads should still emit identifier-resolution attribution"
    );
}

#[test]
fn perf_packet_d_accessor_and_prototype_transition_parity() {
    let source = r#"
globalProbe = 1;
let trace = "";
let first = globalProbe;
Object.defineProperty(globalThis, "globalProbe", {
  get: function() {
    trace = trace + "get|";
    return 9;
  },
  set: function(value) {
    trace = trace + "set:" + value + "|";
  },
  configurable: true
});
let second = globalProbe;
globalProbe = 5;
delete globalThis.globalProbe;
Object.prototype.globalProbe = 13;
let inherited = globalProbe;
delete Object.prototype.globalProbe;
globalProbe = 4;
let final = globalProbe;
trace + first + "|" + second + "|" + inherited + "|" + final;
"#;

    let (value, counters, _) = assert_packet_d_parity(source);
    assert_eq!(value, JsValue::String("get|set:5|1|9|13|4".to_string()));
    assert!(
        counters.global_guard_hits > 0,
        "global own-data reads should still hit packet-D when canonical"
    );
    assert!(
        counters.global_guard_misses > 0,
        "accessor/prototype transitions must force packet-D fallback"
    );
}

#[test]
fn perf_packet_d_mutation_invalidation_parity() {
    let source = r#"
let token = 2;
let sum = token + token + token + token;
{
  let token = 7;
  sum = sum + token;
}
sum = sum + token;
sum;
"#;

    let (value, counters, hotspot) = assert_packet_d_parity(source);
    assert_eq!(value, JsValue::Number(17.0));
    assert!(
        counters.slot_guard_hits > 0,
        "stable lexical reads should produce packet-D slot hits"
    );
    assert!(
        counters.slot_guard_misses > 0,
        "scope mutation should invalidate packet-D slot cache and emit misses"
    );
    assert!(
        hotspot.identifier_resolution > 0,
        "identifier hotspot attribution must remain active with packet-D enabled"
    );
}

#[test]
fn perf_packet_d_slot_revalidation_fallback_parity() {
    let source = r#"
let marker = 1;
let total = marker;
{
  let marker = 9;
  total = total + marker;
}
total = total + marker;
with ({ marker: 20 }) {
  total = total + marker;
}
total;
"#;

    let (value, counters, hotspot) = assert_packet_d_parity(source);
    assert_eq!(value, JsValue::Number(31.0));
    assert!(
        counters.slot_guard_misses > 0,
        "fallback path should still emit packet-D slot misses when guard predicates fail"
    );
    assert!(
        hotspot.identifier_resolution > 0,
        "identifier-resolution hotspot attribution must remain active"
    );
}

#[test]
fn perf_packet_d_packet_g_toggle_matches_packet_d_parity_scripts() {
    let scripts = [
        r#"
globalFast = 5;
let token = 2;
let sum = 0;
for (let i = 0; i < 12; i = i + 1) {
  sum = sum + token;
  sum = sum + globalFast;
}
{
  let token = 9;
  sum = sum + token;
}
sum = sum + token;
let scopeObj = { token: 40 };
with (scopeObj) {
  sum = sum + token;
  token = token + 1;
}
Object.defineProperty(globalThis, "globalFast", {
  get: function() { return 11; },
  set: function(value) { this.__packetDWrite = value; },
  configurable: true
});
sum = sum + globalFast;
globalThis.globalFast = 3;
delete globalThis.globalFast;
globalFast = 4;
sum + globalFast + scopeObj.token + (globalThis.__packetDWrite === 3 ? 1 : 0);
"#,
        r#"
globalProbe = 1;
let trace = "";
let first = globalProbe;
Object.defineProperty(globalThis, "globalProbe", {
  get: function() {
    trace = trace + "get|";
    return 9;
  },
  set: function(value) {
    trace = trace + "set:" + value + "|";
  },
  configurable: true
});
let second = globalProbe;
globalProbe = 5;
delete globalThis.globalProbe;
Object.prototype.globalProbe = 13;
let inherited = globalProbe;
delete Object.prototype.globalProbe;
globalProbe = 4;
let final = globalProbe;
trace + first + "|" + second + "|" + inherited + "|" + final;
"#,
        r#"
let marker = 1;
let total = marker;
{
  let marker = 9;
  total = total + marker;
}
total = total + marker;
with ({ marker: 20 }) {
  total = total + marker;
}
total;
"#,
    ];

    for source in scripts {
        let (_value, counters, hotspot) = assert_packet_g_toggle_parity(source, true);
        assert!(
            counters.name_guard_misses > 0,
            "packet-g should record fallback misses on packet-d parity scripts"
        );
        assert!(
            hotspot.packet_g_name_guard_misses > 0,
            "packet-g hotspot attribution should mirror miss coverage on parity scripts"
        );
    }
}

#[test]
fn perf_packet_d_packet_g_identifier_guard_counter_coverage() {
    let source = r#"
let local = 2;
let total = 0;
for (let i = 0; i < 12; i = i + 1) {
  total = total + local;
}
for (let j = 0; j < 5; j = j + 1) {
  {
    let local = j;
    total = total + local;
  }
}
globalProbe = 1;
Object.defineProperty(globalThis, "globalProbe", {
  get: function() { return 8; },
  configurable: true
});
total = total + globalProbe;
delete globalThis.globalProbe;
Object.prototype.globalProbe = 4;
total = total + globalProbe;
delete Object.prototype.globalProbe;
with ({ local: 9 }) {
  total = total + local;
}
total = total + (typeof neverDeclared === "undefined" ? 1 : 0);
total;
"#;

    let (_value, counters, hotspot) = assert_packet_g_toggle_parity(source, false);
    assert!(
        counters.name_guard_hits > 0,
        "stable lexical identifier loops should hit packet-g name guard"
    );
    assert!(
        counters.name_guard_misses > 0,
        "with/prototype/accessor/unknown identifier paths must force packet-g fallback misses"
    );
    assert!(
        counters.name_guard_revalidate_hits + counters.name_guard_revalidate_misses > 0,
        "scope generation churn should exercise packet-g revalidate buckets"
    );
    assert!(
        hotspot.packet_g_name_guard_hits > 0,
        "packet-g hit counters must flow into hotspot attribution"
    );
    assert!(
        hotspot.packet_g_name_guard_misses > 0,
        "packet-g miss counters must flow into hotspot attribution"
    );
    assert!(
        hotspot.packet_g_name_guard_revalidate_hits + hotspot.packet_g_name_guard_revalidate_misses
            > 0,
        "packet-g revalidate counters must flow into hotspot attribution"
    );
}

#[test]
fn perf_packet_d_packet_g_scope_generation_invalidation_parity() {
    let source = r#"
let marker = 1;
let total = 0;
for (let i = 0; i < 6; i = i + 1) {
  total = total + marker;
  {
    let marker = i;
    total = total + marker;
  }
}
total = total + marker;
total;
"#;

    let (value, counters, _hotspot) = assert_packet_g_toggle_parity(source, false);
    assert_eq!(value, JsValue::Number(22.0));
    assert!(
        counters.name_guard_revalidate_misses > 0,
        "scope-generation invalidation should reject stale packet-g name cache entries"
    );
}
