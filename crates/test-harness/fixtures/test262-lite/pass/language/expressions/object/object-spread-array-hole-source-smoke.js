/*---
description: object spread copies sparse array own indices only
---*/

var arr = [];
arr[2] = 3;

var out = { ...arr };
assert.sameValue(out[0], undefined);
assert.sameValue(out[2], 3);
assert.sameValue(Object.keys(out).join(","), "2");