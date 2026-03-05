/*---
description: object spread snapshot is independent from source mutation
---*/

var source = { value: 1 };
var out = { ...source };
source.value = 2;

assert.sameValue(out.value, 1);
assert.sameValue(source.value, 2);