/*---
description: object spread can overwrite previous copied keys with later spread
---*/

var out = { ...{ a: 1, b: 2 }, ...{ a: 9 } };

assert.sameValue(out.a, 9);
assert.sameValue(out.b, 2);