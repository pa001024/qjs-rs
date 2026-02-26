/*---
description: array baseline smoke
---*/

var arr = [1, 2, 3];
arr.length = 2;
assert.sameValue(Array.isArray(arr), true);
assert.sameValue(arr.length, 2);
assert.sameValue(arr[2], undefined);
