/*---
description: object spread overwrite order
---*/

var out = { a: 1, ...{ a: 2, b: 4 }, a: 3 };

assert.sameValue(out.a, 3);
assert.sameValue(out.b, 4);