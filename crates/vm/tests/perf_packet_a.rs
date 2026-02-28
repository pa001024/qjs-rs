#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::JsValue;
use vm::{PacketAFastPathCounters, Vm};

fn run_script(source: &str, packet_a_enabled: bool) -> (JsValue, PacketAFastPathCounters) {
    let script = parse_script(source).expect("script should parse");
    let chunk = compile_script(&script);
    let mut vm = Vm::default();
    vm.set_packet_a_fast_path_enabled(packet_a_enabled);
    vm.set_packet_a_fast_path_metrics_enabled(true);
    let value = vm.execute(&chunk).expect("script should execute");
    (value, vm.packet_a_fast_path_counters())
}

#[test]
fn packet_a_numeric_fast_path_parity() {
    let source = r#"
let total = 0;
for (let i = 0; i < 64; i = i + 1) {
  total = total + i;
  total = total - 1 + 1;
  total = total * 1;
  total = total / 1;
}
let boolMath = true + 2;
let concat = "v" + 1 + 2;
total + boolMath + (concat === "v12" ? 1 : 0);
"#;

    let (slow_value, slow_counters) = run_script(source, false);
    let (fast_value, fast_counters) = run_script(source, true);

    assert_eq!(slow_value, JsValue::Number(2020.0));
    assert_eq!(fast_value, slow_value);
    assert_eq!(
        slow_counters,
        PacketAFastPathCounters::default(),
        "fast-path counters should stay zero when packet A is disabled"
    );
    assert!(
        fast_counters.numeric_guard_hits > 0,
        "numeric guard hits should be recorded for number/bool arithmetic"
    );
    assert!(
        fast_counters.numeric_guard_misses > 0,
        "numeric guard misses should be recorded for non-fast-path add semantics"
    );
}

#[test]
fn packet_a_binding_cache_scope_invalidation() {
    let source = r#"
let seed = 1;
let acc = 0;
for (let i = 0; i < 16; i = i + 1) {
  acc = acc + seed;
}
function makeReader() {
  let seed = 40;
  return function() { return seed; };
}
let readCaptured = makeReader();
{
  let seed = 2;
  acc = acc + seed;
}
acc = acc + readCaptured();
acc = acc + seed;
acc;
"#;

    let (slow_value, slow_counters) = run_script(source, false);
    let (fast_value, fast_counters) = run_script(source, true);

    assert_eq!(slow_value, JsValue::Number(59.0));
    assert_eq!(fast_value, slow_value);
    assert_eq!(
        slow_counters,
        PacketAFastPathCounters::default(),
        "disabled packet A should not emit binding counters"
    );
    assert!(
        fast_counters.binding_guard_hits > 0,
        "binding cache should record guard hits under loop-heavy lookups"
    );
    assert!(
        fast_counters.binding_guard_misses > 0,
        "scope mutation and first-touch lookups should record guard misses"
    );
}

#[test]
fn packet_a_binding_cache_with_scope_fallback() {
    let source = r#"
var globalHits = 3;
var obj = { globalHits: 9 };
var sum = 0;
for (var i = 0; i < 8; i = i + 1) {
  sum = sum + globalHits;
}
sloppyWrite = 4;
sum = sum + sloppyWrite;
with (obj) {
  sum = sum + globalHits;
  globalHits = globalHits + 1;
}
sum = sum + globalHits;
sum = sum + obj.globalHits;
sum;
"#;

    let (slow_value, _) = run_script(source, false);
    let (fast_value, fast_counters) = run_script(source, true);

    assert_eq!(slow_value, JsValue::Number(50.0));
    assert_eq!(fast_value, slow_value);
    assert!(
        fast_counters.binding_guard_misses > 0,
        "with/global-property fallthrough should invalidate guarded binding paths"
    );
}
