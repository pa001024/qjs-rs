#![forbid(unsafe_code)]

use bytecode::compile_script;
use parser::parse_script;
use runtime::{JsValue, NativeFunction, Realm};
use vm::Vm;

#[test]
fn weak_collection_constructor_identity() {
    let script = parse_script(
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
    )
    .expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    realm.define_global(
        "Map",
        JsValue::NativeFunction(NativeFunction::MapConstructor),
    );
    realm.define_global(
        "Set",
        JsValue::NativeFunction(NativeFunction::SetConstructor),
    );
    realm.define_global(
        "WeakMap",
        JsValue::NativeFunction(NativeFunction::WeakMapConstructor),
    );
    realm.define_global(
        "WeakSet",
        JsValue::NativeFunction(NativeFunction::WeakSetConstructor),
    );
    realm.define_global(
        "TypeError",
        JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
    );
    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}

#[test]
fn collection_semantics_same_value_zero_and_live_iteration() {
    let script = parse_script(
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
         ok = ok && set.has(nan) && set.has(+0) && setSeen.length === 3 && setSeen[2] === 1; \
         var wm = new WeakMap(); \
         var ws = new WeakSet(); \
         var key = {}; \
         wm.set(key, 42); \
         ws.add(key); \
         ok = ok && wm.get(key) === 42 && wm.has(key) && wm.delete(key) && !wm.has(key); \
         ok = ok && ws.has(key) && ws.delete(key) && !ws.has(key); \
         var weakMapSetTypeError = false; \
         var weakMapGetTypeError = false; \
         var weakMapHasTypeError = false; \
         var weakMapDeleteTypeError = false; \
         try { wm.set(1, 1); } catch (e) { weakMapSetTypeError = e instanceof TypeError; } \
         try { wm.get(1); } catch (e) { weakMapGetTypeError = e instanceof TypeError; } \
         try { wm.has(1); } catch (e) { weakMapHasTypeError = e instanceof TypeError; } \
         try { wm.delete(1); } catch (e) { weakMapDeleteTypeError = e instanceof TypeError; } \
         var weakSetAddTypeError = false; \
         var weakSetHasTypeError = false; \
         var weakSetDeleteTypeError = false; \
         try { ws.add(1); } catch (e) { weakSetAddTypeError = e instanceof TypeError; } \
         try { ws.has(1); } catch (e) { weakSetHasTypeError = e instanceof TypeError; } \
         try { ws.delete(1); } catch (e) { weakSetDeleteTypeError = e instanceof TypeError; } \
         ok = ok && weakMapSetTypeError && weakMapGetTypeError && weakMapHasTypeError && weakMapDeleteTypeError; \
         ok = ok && weakSetAddTypeError && weakSetHasTypeError && weakSetDeleteTypeError; \
         var weakMapPulls = 0; \
         var weakMapFailFast = false; \
         var weakMapIterable = { \
           [Symbol.iterator]: function() { \
             return { \
               next: function() { \
                 weakMapPulls = weakMapPulls + 1; \
                 if (weakMapPulls === 1) return { value: [{}, 1], done: false }; \
                 if (weakMapPulls === 2) return { value: 1, done: false }; \
                 return { value: undefined, done: true }; \
               } \
             }; \
           } \
         }; \
         try { new WeakMap(weakMapIterable); } catch (e) { weakMapFailFast = e instanceof TypeError; } \
         var weakSetPulls = 0; \
         var weakSetFailFast = false; \
         var weakSetIterable = { \
           [Symbol.iterator]: function() { \
             return { \
               next: function() { \
                 weakSetPulls = weakSetPulls + 1; \
                 if (weakSetPulls === 1) return { value: {}, done: false }; \
                 if (weakSetPulls === 2) return { value: 1, done: false }; \
                 return { value: undefined, done: true }; \
               } \
             }; \
           } \
         }; \
         try { new WeakSet(weakSetIterable); } catch (e) { weakSetFailFast = e instanceof TypeError; } \
         ok && weakMapFailFast && weakMapPulls === 2 && weakSetFailFast && weakSetPulls === 2;",
    )
    .expect("script should parse");
    let chunk = compile_script(&script);
    let mut realm = Realm::default();
    realm.define_global(
        "Map",
        JsValue::NativeFunction(NativeFunction::MapConstructor),
    );
    realm.define_global(
        "Set",
        JsValue::NativeFunction(NativeFunction::SetConstructor),
    );
    realm.define_global(
        "WeakSet",
        JsValue::NativeFunction(NativeFunction::WeakSetConstructor),
    );
    realm.define_global(
        "WeakMap",
        JsValue::NativeFunction(NativeFunction::WeakMapConstructor),
    );
    realm.define_global(
        "Symbol",
        JsValue::NativeFunction(NativeFunction::SymbolConstructor),
    );
    realm.define_global(
        "TypeError",
        JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
    );
    let mut vm = Vm::default();
    assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
}
