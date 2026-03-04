/*---
description: object spread applies properties in source order
---*/

var out = { ...{ a: 1, b: 2 }, b: 3, ...{ c: 4 }, a: 5 };

assert.sameValue(out.a, 5);
assert.sameValue(out.b, 3);
assert.sameValue(out.c, 4);