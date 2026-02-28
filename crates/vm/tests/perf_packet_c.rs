#![forbid(unsafe_code)]

use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use vm::{PacketCFastPathCounters, Vm};

fn run_script(source: &str, packet_c_enabled: bool) -> (JsValue, PacketCFastPathCounters) {
    let script = parse_script(source).expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    install_baseline(&mut realm);

    let mut vm = Vm::default();
    vm.set_packet_c_fast_path_enabled(packet_c_enabled);
    vm.set_packet_c_fast_path_metrics_enabled(true);

    let value = vm
        .execute_in_realm(&chunk, &realm)
        .expect("script should execute");
    (value, vm.packet_c_fast_path_counters())
}

fn assert_packet_c_parity(source: &str) -> (JsValue, PacketCFastPathCounters) {
    let (slow_value, slow_counters) = run_script(source, false);
    let (fast_value, fast_counters) = run_script(source, true);

    assert_eq!(fast_value, slow_value);
    assert_eq!(
        slow_counters,
        PacketCFastPathCounters::default(),
        "packet-C counters must stay zero when optimization is disabled"
    );

    (fast_value, fast_counters)
}

#[test]
fn packet_c_identifier_resolution_guarding() {
    let source = r#"
globalFast = 9;
let total = 0;
for (let i = 0; i < 48; i = i + 1) {
  total = total + i;
  total = total + globalFast;
}
let scopeObj = { globalFast: 30 };
with (scopeObj) {
  total = total + globalFast;
  globalFast = globalFast + 1;
}
Object.defineProperty(globalThis, "globalFast", {
  get: function() { return 12; },
  set: function(value) { this.__globalFastWrite = value; },
  configurable: true
});
total = total + globalFast;
globalThis.globalFast = 4;
delete globalThis.globalFast;
globalFast = 5;
total + globalFast + scopeObj.globalFast + (globalThis.__globalFastWrite === 4 ? 1 : 0);
"#;

    let (value, counters) = assert_packet_c_parity(source);
    assert_eq!(value, JsValue::Number(1639.0));
    assert!(
        counters.identifier_guard_hits > 0,
        "loop-heavy lexical lookups should trigger packet-C identifier hits"
    );
    assert!(
        counters.identifier_guard_misses > 0,
        "with scope transitions should trigger packet-C identifier guard misses"
    );
    assert!(
        counters.global_guard_hits > 0,
        "global own-data identifier reads should trigger packet-C global hits"
    );
    assert!(
        counters.global_guard_misses > 0,
        "accessor/prototype or missing-property cases must trigger packet-C global misses"
    );
}

#[test]
fn perf_packet_c_typeof_unknown_identifier_parity() {
    let source = r#"typeof neverDeclared;"#;

    let (value, counters) = assert_packet_c_parity(source);
    assert_eq!(value, JsValue::String("undefined".to_string()));
    assert!(
        counters.identifier_guard_misses > 0,
        "unknown identifiers should route through packet-C miss path"
    );
}

#[test]
fn perf_packet_c_with_scope_lookup_parity() {
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

    let (value, counters) = assert_packet_c_parity(source);
    assert_eq!(value, JsValue::Number(15.0));
    assert!(
        counters.identifier_guard_misses > 0,
        "with lookups must bypass packet-C identifier cache"
    );
}

#[test]
fn perf_packet_c_accessor_and_prototype_transition_parity() {
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

    let (value, counters) = assert_packet_c_parity(source);
    assert_eq!(value, JsValue::String("get|set:5|1|9|13|4".to_string()));
    assert!(
        counters.global_guard_hits > 0,
        "global own-data reads should still hit packet-C when canonical"
    );
    assert!(
        counters.global_guard_misses > 0,
        "accessor/prototype transitions must force packet-C fallback"
    );
}

#[test]
fn perf_packet_c_mutation_invalidation_parity() {
    let source = r#"
let token = 2;
let sum = 0;
for (let i = 0; i < 8; i = i + 1) {
  sum = sum + token;
}
{
  let token = 7;
  sum = sum + token;
}
sum = sum + token;
sum;
"#;

    let (value, counters) = assert_packet_c_parity(source);
    assert_eq!(value, JsValue::Number(25.0));
    assert!(
        counters.identifier_guard_hits > 0,
        "stable lexical reads should produce packet-C identifier hits"
    );
    assert!(
        counters.identifier_guard_misses > 0,
        "scope mutation should invalidate packet-C caches and emit misses"
    );
}
