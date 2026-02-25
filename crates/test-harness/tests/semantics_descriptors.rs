#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::run_script;

fn assert_script(source: &str, expected: JsValue) {
    let result = run_script(source, &[]);
    assert_eq!(result, Ok(expected), "unexpected result for script:\n{source}");
}

#[test]
fn non_configurable_data_property_rejects_value_and_enumerable_changes() {
    assert_script(
        "var obj = {}; \
         Object.defineProperty(obj, 'x', { value: 1, writable: false, enumerable: false, configurable: false }); \
         var valueError = false; \
         var enumerableError = false; \
         try { Object.defineProperty(obj, 'x', { value: 2 }); } catch (e) { valueError = e instanceof TypeError; } \
         try { Object.defineProperty(obj, 'x', { enumerable: true }); } catch (e) { enumerableError = e instanceof TypeError; } \
         var desc = Object.getOwnPropertyDescriptor(obj, 'x'); \
         valueError && enumerableError && desc.value === 1 && desc.writable === false && desc.enumerable === false && desc.configurable === false;",
        JsValue::Bool(true),
    );
}

#[test]
fn descriptor_cannot_mix_accessor_and_data_fields() {
    assert_script(
        "var threw = false; \
         try { Object.defineProperty({}, 'x', { value: 1, get: function() { return 1; } }); } \
         catch (e) { threw = e instanceof TypeError; } \
         threw;",
        JsValue::Bool(true),
    );
}

#[test]
fn non_configurable_accessor_rejects_getter_replacement() {
    assert_script(
        "var obj = {}; \
         var getter1 = function() { return 1; }; \
         var getter2 = function() { return 2; }; \
         Object.defineProperty(obj, 'x', { get: getter1, configurable: false }); \
         var threw = false; \
         try { Object.defineProperty(obj, 'x', { get: getter2 }); } catch (e) { threw = e instanceof TypeError; } \
         var desc = Object.getOwnPropertyDescriptor(obj, 'x'); \
         threw && desc.get === getter1 && desc.configurable === false && !('value' in desc) && !('writable' in desc);",
        JsValue::Bool(true),
    );
}

#[test]
fn define_properties_prevalidates_mixed_descriptors_before_mutation() {
    assert_script(
        "var obj = {}; \
         var threw = false; \
         try { \
           Object.defineProperties(obj, { \
             a: { value: 1, enumerable: true }, \
             z: { get: 1 } \
           }); \
         } catch (e) { threw = e instanceof TypeError; } \
         threw && !Object.prototype.hasOwnProperty.call(obj, 'a') && !Object.prototype.hasOwnProperty.call(obj, 'z');",
        JsValue::Bool(true),
    );
}

#[test]
fn descriptor_readback_matches_data_and_accessor_state() {
    assert_script(
        "var obj = {}; \
         var getter = function() { return 7; }; \
         Object.defineProperty(obj, 'a', { value: 3, writable: false, enumerable: true, configurable: false }); \
         Object.defineProperty(obj, 'b', { get: getter, set: undefined, enumerable: false, configurable: true }); \
         var a = Object.getOwnPropertyDescriptor(obj, 'a'); \
         var b = Object.getOwnPropertyDescriptor(obj, 'b'); \
         var all = Object.getOwnPropertyDescriptors(obj); \
         a.value === 3 && a.writable === false && a.enumerable === true && a.configurable === false && \
         b.get === getter && b.set === undefined && b.enumerable === false && b.configurable === true && \
         !('value' in b) && !('writable' in b) && \
         all.a.value === 3 && all.a.writable === false && all.b.get === getter && all.b.enumerable === false;",
        JsValue::Bool(true),
    );
}

#[test]
fn array_non_writable_length_rejects_index_extension() {
    assert_script(
        "var arr = [0]; \
         Object.defineProperty(arr, 'length', { writable: false }); \
         var threw = false; \
         try { Object.defineProperty(arr, '1', { value: 1 }); } catch (e) { threw = e instanceof TypeError; } \
         threw && arr.length === 1 && arr[1] === undefined;",
        JsValue::Bool(true),
    );
}

#[test]
fn array_length_shrink_failure_preserves_index_and_length_descriptor() {
    assert_script(
        "var arr = [0, 1, 2]; \
         Object.defineProperty(arr, '2', { value: 2, writable: true, enumerable: true, configurable: false }); \
         var threw = false; \
         try { Object.defineProperty(arr, 'length', { value: 1, writable: false }); } catch (e) { threw = e instanceof TypeError; } \
         var desc = Object.getOwnPropertyDescriptor(arr, 'length'); \
         threw && arr.length === 3 && arr[2] === 2 && desc.value === 3 && desc.writable === false && desc.enumerable === false && desc.configurable === false;",
        JsValue::Bool(true),
    );
}

#[test]
fn define_properties_readback_matches_written_attributes() {
    assert_script(
        "var obj = {}; \
         var count = 0; \
         Object.defineProperties(obj, { \
           x: { value: 1, writable: false, enumerable: false, configurable: true }, \
           y: { get: function() { count = count + 1; return 2; }, enumerable: true, configurable: false } \
         }); \
         var x = Object.getOwnPropertyDescriptor(obj, 'x'); \
         var y = Object.getOwnPropertyDescriptor(obj, 'y'); \
         var all = Object.getOwnPropertyDescriptors(obj); \
         x.value === 1 && x.writable === false && x.enumerable === false && x.configurable === true && \
         typeof y.get === 'function' && y.enumerable === true && y.configurable === false && \
         all.y.enumerable === true && all.x.configurable === true && obj.y === 2 && count === 1;",
        JsValue::Bool(true),
    );
}
