#![forbid(unsafe_code)]

use builtins::install_baseline;
use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, Realm};
use vm::{PacketBFastPathCounters, Vm};

fn run_script(source: &str, packet_b_enabled: bool) -> (JsValue, PacketBFastPathCounters) {
    let script = parse_script(source).expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    install_baseline(&mut realm);
    let mut vm = Vm::default();
    vm.set_packet_b_fast_path_enabled(packet_b_enabled);
    vm.set_packet_b_fast_path_metrics_enabled(true);
    let value = vm
        .execute_in_realm(&chunk, &realm)
        .expect("script should execute");
    (value, vm.packet_b_fast_path_counters())
}

fn assert_packet_b_parity(source: &str) -> (JsValue, PacketBFastPathCounters) {
    let (slow_value, slow_counters) = run_script(source, false);
    let (fast_value, fast_counters) = run_script(source, true);
    assert_eq!(fast_value, slow_value);
    assert_eq!(
        slow_counters,
        PacketBFastPathCounters::default(),
        "packet-B counters must stay zero when optimization is disabled"
    );
    (fast_value, fast_counters)
}

fn assert_dense_loop_parity_and_hits() {
    let source = r#"
let arr = [];
for (let i = 0; i < 64; i = i + 1) {
  arr[i] = i;
}
let sum = 0;
for (let i = 0; i < arr.length; i = i + 1) {
  sum = sum + arr[i];
}
sum;
"#;

    let (value, counters) = assert_packet_b_parity(source);
    assert_eq!(value, JsValue::Number(2016.0));
    assert!(
        counters.dense_array_get_guard_hits > 0,
        "dense array reads should trigger packet-B get hits"
    );
    assert!(
        counters.dense_array_set_guard_hits > 0,
        "dense array writes should trigger packet-B set hits"
    );
}

#[test]
fn packet_b_array_dense_index_fast_path_guarding() {
    assert_dense_loop_parity_and_hits();
}

#[test]
fn perf_packet_b_dense_loop_parity_and_hits() {
    assert_dense_loop_parity_and_hits();
}

#[test]
fn perf_packet_b_parity_holes_and_inherited_indices() {
    let source = r#"
let arr = [1, 2, 3, 4];
delete arr[1];
Array.prototype[1] = 20;
let inherited = arr[1];
delete Array.prototype[1];
let hole = arr[1] === undefined ? 1 : 0;
inherited + hole;
"#;

    let (value, counters) = assert_packet_b_parity(source);
    assert_eq!(value, JsValue::Number(21.0));
    assert!(
        counters.dense_array_get_guard_misses > 0,
        "holes/inherited index lookups must fall back to canonical path"
    );
}

#[test]
fn perf_packet_b_parity_accessor_properties() {
    let source = r#"
let arr = [5, 6, 7];
let log = [];
Object.defineProperty(arr, "1", {
  get: function() {
    log.push("get");
    return 40;
  },
  set: function(value) {
    log.push("set:" + value);
  },
  configurable: true
});
let first = arr[1];
arr[1] = 99;
log.push(String(arr[1]));
log.join("|") + ":" + first;
"#;

    let (value, counters) = assert_packet_b_parity(source);
    assert_eq!(value, JsValue::String("get|set:99|get|40:40".to_string()));
    assert!(
        counters.dense_array_get_guard_misses > 0,
        "accessor-backed reads must not use dense fast path"
    );
    assert!(
        counters.dense_array_set_guard_misses > 0,
        "accessor-backed writes must not use dense fast path"
    );
}

#[test]
fn perf_packet_b_parity_sparse_arrays() {
    let source = r#"
let arr = [];
arr[0] = 1;
arr[5] = 7;
let before = arr.length;
let hole = arr[2] === undefined ? 1 : 0;
arr[3] = 9;
let after = arr.length;
before + after + hole + arr[5];
"#;

    let (value, counters) = assert_packet_b_parity(source);
    assert_eq!(value, JsValue::Number(20.0));
    assert!(
        counters.dense_array_set_guard_misses > 0,
        "sparse writes must fall back to canonical set semantics"
    );
}

#[test]
fn perf_packet_b_parity_prototype_mutation_error_order() {
    let source = r#"
let arr = [1, 2];
let trace = "";
Object.defineProperty(Array.prototype, "4", {
  get: function() {
    trace = trace + "proto-get|";
    throw new TypeError("proto");
  },
  configurable: true
});
let threw = false;
try {
  arr[4];
} catch (err) {
  threw = err instanceof TypeError;
  trace = trace + "caught|";
}
delete Array.prototype[4];
let setterTotal = 0;
Object.defineProperty(Array.prototype, "2", {
  set: function(value) {
    setterTotal = setterTotal + value;
    trace = trace + "set|";
  },
  configurable: true
});
arr[2] = 5;
delete Array.prototype[2];
trace + String(threw) + "|" + setterTotal;
"#;

    let (value, counters) = assert_packet_b_parity(source);
    assert_eq!(
        value,
        JsValue::String("proto-get|caught|set|true|5".to_string())
    );
    assert!(
        counters.dense_array_get_guard_misses > 0,
        "prototype-index getter lookups must fall back"
    );
    assert!(
        counters.dense_array_set_guard_misses > 0,
        "prototype-index setter writes must fall back"
    );
}
