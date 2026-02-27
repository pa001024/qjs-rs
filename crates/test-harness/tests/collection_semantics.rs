#![forbid(unsafe_code)]

use runtime::JsValue;
use test_harness::run_script;

fn assert_script(source: &str, expected: JsValue) {
    assert_eq!(run_script(source, &[]), Ok(expected));
}

#[test]
fn collection_constructor_identity_baseline() {
    assert_script(
        "var ok = true; \
         ok = ok && WeakMap !== Map && WeakSet !== Set; \
         ok = ok && WeakMap.name === 'WeakMap' && WeakSet.name === 'WeakSet'; \
         ok = ok && WeakMap.prototype !== Map.prototype && WeakSet.prototype !== Set.prototype; \
         ok = ok && WeakMap.prototype.constructor === WeakMap && WeakSet.prototype.constructor === WeakSet; \
         ok = ok && typeof WeakMap.prototype.set === 'function' && typeof WeakMap.prototype.get === 'function' && typeof WeakMap.prototype.has === 'function' && typeof WeakMap.prototype.delete === 'function'; \
         ok = ok && typeof WeakSet.prototype.add === 'function' && typeof WeakSet.prototype.has === 'function' && typeof WeakSet.prototype.delete === 'function'; \
         var weakMapRequiresNew = false; \
         var weakSetRequiresNew = false; \
         try { WeakMap(); } catch (e) { weakMapRequiresNew = e instanceof TypeError; } \
         try { WeakSet(); } catch (e) { weakSetRequiresNew = e instanceof TypeError; } \
         ok && weakMapRequiresNew && weakSetRequiresNew;",
        JsValue::Bool(true),
    );
}

#[test]
fn map_set_same_value_zero_and_live_iteration_baseline() {
    assert_script(
        "var ok = true; \
         var map = new Map(); \
         var nan = 0 / 0; \
         map.set(nan, 'nan'); \
         map.set(-0, 'zero'); \
         ok = ok && map.has(nan) && map.get(nan) === 'nan' && map.get(+0) === 'zero'; \
         var mapOrder = []; \
         var map2 = new Map([['a', 1], ['b', 2]]); \
         map2.set('a', 3); \
         map2.delete('a'); \
         map2.set('a', 4); \
         map2.forEach(function(value, key) { \
           mapOrder.push(key + ':' + value); \
           if (key === 'b') { map2.set('c', 5); } \
         }); \
         ok = ok && mapOrder.length === 3 && mapOrder[0] === 'b:2' && mapOrder[1] === 'a:4' && mapOrder[2] === 'c:5'; \
         var set = new Set([nan, -0]); \
         var setSeen = []; \
         set.forEach(function(value) { \
           setSeen.push(value); \
           if (setSeen.length === 1) { set.add(1); } \
         }); \
         ok && set.has(nan) && set.has(+0) && setSeen.length === 3 && setSeen[2] === 1;",
        JsValue::Bool(true),
    );
}

#[test]
fn weakmap_key_type_errors_and_constructor_fail_fast() {
    assert_script(
        "var ok = true; \
         var wm = new WeakMap(); \
         var key = {}; \
         wm.set(key, 42); \
         ok = ok && wm.get(key) === 42 && wm.has(key) && wm.delete(key) && !wm.has(key); \
         var setTypeError = false; \
         var getTypeError = false; \
         var hasTypeError = false; \
         var deleteTypeError = false; \
         try { wm.set(1, 1); } catch (e) { setTypeError = e instanceof TypeError; } \
         try { wm.get(1); } catch (e) { getTypeError = e instanceof TypeError; } \
         try { wm.has(1); } catch (e) { hasTypeError = e instanceof TypeError; } \
         try { wm.delete(1); } catch (e) { deleteTypeError = e instanceof TypeError; } \
         var pulls = 0; \
         var failFast = false; \
         var iterable = { \
           [Symbol.iterator]: function() { \
             return { \
               next: function() { \
                 pulls = pulls + 1; \
                 if (pulls === 1) return { value: [{}, 1], done: false }; \
                 if (pulls === 2) return { value: 1, done: false }; \
                 return { value: undefined, done: true }; \
               } \
             }; \
           } \
         }; \
         try { new WeakMap(iterable); } catch (e) { failFast = e instanceof TypeError; } \
         ok && setTypeError && getTypeError && hasTypeError && deleteTypeError && failFast && pulls === 2;",
        JsValue::Bool(true),
    );
}

#[test]
fn weakset_key_type_errors_and_constructor_fail_fast() {
    assert_script(
        "var ok = true; \
         var ws = new WeakSet(); \
         var key = {}; \
         ws.add(key); \
         ok = ok && ws.has(key) && ws.delete(key) && !ws.has(key); \
         var addTypeError = false; \
         var hasTypeError = false; \
         var deleteTypeError = false; \
         try { ws.add(1); } catch (e) { addTypeError = e instanceof TypeError; } \
         try { ws.has(1); } catch (e) { hasTypeError = e instanceof TypeError; } \
         try { ws.delete(1); } catch (e) { deleteTypeError = e instanceof TypeError; } \
         var pulls = 0; \
         var failFast = false; \
         var iterable = { \
           [Symbol.iterator]: function() { \
             return { \
               next: function() { \
                 pulls = pulls + 1; \
                 if (pulls === 1) return { value: {}, done: false }; \
                 if (pulls === 2) return { value: 1, done: false }; \
                 return { value: undefined, done: true }; \
               } \
             }; \
           } \
         }; \
         try { new WeakSet(iterable); } catch (e) { failFast = e instanceof TypeError; } \
         ok && addTypeError && hasTypeError && deleteTypeError && failFast && pulls === 2;",
        JsValue::Bool(true),
    );
}
