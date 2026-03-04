/*---
description: JSON.stringify handles Symbol values and proxy-backed object/array inputs
---*/
var sym = Symbol("desc");
var objWithSymbolValues = { key: sym };
objWithSymbolValues[sym] = 1;

assert.sameValue(JSON.stringify(sym), undefined);
assert.sameValue(JSON.stringify([sym]), "[null]");
assert.sameValue(JSON.stringify(objWithSymbolValues), "{}");

var arrayProxy = new Proxy([], {
  get: function (_target, key) {
    if (key === "length") {
      return 2;
    }
    return Number(key);
  }
});

assert.sameValue(JSON.stringify(arrayProxy), "[0,1]");

var objectProxy = new Proxy({}, {
  ownKeys: function () {
    return ["a", "b"];
  },
  getOwnPropertyDescriptor: function () {
    return { value: 1, writable: true, enumerable: true, configurable: true };
  },
  get: function () {
    return 1;
  }
});

assert.sameValue(JSON.stringify(objectProxy), '{"a":1,"b":1}');

