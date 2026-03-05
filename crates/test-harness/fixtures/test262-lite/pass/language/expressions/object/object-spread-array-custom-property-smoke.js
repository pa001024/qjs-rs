/*---
description: object spread copies array indices and custom enumerable fields
---*/

var arr = [10, 20];
arr.extra = 12;
var out = { ...arr };

assert.sameValue(out[0], 10);
assert.sameValue(out[1], 20);
assert.sameValue(out.extra, 12);