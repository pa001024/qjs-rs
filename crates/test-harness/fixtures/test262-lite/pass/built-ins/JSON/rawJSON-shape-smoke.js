/*---
description: JSON.rawJSON returns a frozen null-prototype object with rawJSON data property
---*/
var raw = JSON.rawJSON(1);
assert.sameValue(Object.getPrototypeOf(raw), null);
assert.sameValue(Object.isFrozen(raw), true);
assert.sameValue(Object.getOwnPropertyNames(raw).length, 1);
assert.sameValue(Object.getOwnPropertyNames(raw)[0], "rawJSON");

var descriptor = Object.getOwnPropertyDescriptor(raw, "rawJSON");
assert.sameValue(descriptor.value, "1");
assert.sameValue(descriptor.enumerable, true);
assert.sameValue(descriptor.writable, false);
assert.sameValue(descriptor.configurable, false);
