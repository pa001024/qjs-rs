/*---
description: JSON.rawJSON and JSON.isRawJSON builtin descriptors and callability
---*/
assert.sameValue(typeof JSON.rawJSON, "function");
assert.sameValue(typeof JSON.isRawJSON, "function");
assert.sameValue(Object.isExtensible(JSON.rawJSON), true);
assert.sameValue(Object.isExtensible(JSON.isRawJSON), true);
assert.sameValue(Object.getPrototypeOf(JSON.rawJSON), Function.prototype);
assert.sameValue(Object.getPrototypeOf(JSON.isRawJSON), Function.prototype);

var rawJSONDescriptor = Object.getOwnPropertyDescriptor(JSON, "rawJSON");
assert.sameValue(rawJSONDescriptor.enumerable, false);
assert.sameValue(rawJSONDescriptor.writable, true);
assert.sameValue(rawJSONDescriptor.configurable, true);

var isRawJSONDescriptor = Object.getOwnPropertyDescriptor(JSON, "isRawJSON");
assert.sameValue(isRawJSONDescriptor.enumerable, false);
assert.sameValue(isRawJSONDescriptor.writable, true);
assert.sameValue(isRawJSONDescriptor.configurable, true);

assert.sameValue(JSON.rawJSON.length, 1);
assert.sameValue(JSON.rawJSON.name, "rawJSON");
assert.sameValue(JSON.isRawJSON.length, 1);
assert.sameValue(JSON.isRawJSON.name, "isRawJSON");
assert.sameValue(Object.getOwnPropertyDescriptor(JSON.rawJSON, "prototype"), undefined);
assert.sameValue(Object.getOwnPropertyDescriptor(JSON.isRawJSON, "prototype"), undefined);

assert.throws(TypeError, function () {
  new JSON.rawJSON("1");
});
assert.throws(TypeError, function () {
  new JSON.isRawJSON({});
});

