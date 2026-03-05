/*---
description: JSON.parse reviver ignores failed create/delete on non-configurable properties
---*/
var arr = JSON.parse("[1, 2]", function (key, value) {
  if (key === "0") {
    Object.defineProperty(this, "1", { configurable: false });
  }
  if (key === "1") {
    return 22;
  }
  return value;
});

assert.sameValue(arr[0], 1);
assert.sameValue(arr[1], 2);

var obj = JSON.parse('{"a": 1, "b": 2}', function (key, value) {
  if (key === "a") {
    Object.defineProperty(this, "b", { configurable: false });
  }
  if (key === "b") {
    return undefined;
  }
  return value;
});

assert.sameValue(obj.a, 1);
assert.sameValue(obj.hasOwnProperty("b"), true);
assert.sameValue(obj.b, 2);

