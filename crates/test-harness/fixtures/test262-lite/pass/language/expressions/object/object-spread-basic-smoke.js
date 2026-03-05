/*---
description: object spread basic copy semantics
---*/

var source = { a: 1, b: 2 };
var out = { prefix: 0, ...source, suffix: 3 };

assert.sameValue(out.prefix, 0);
assert.sameValue(out.a, 1);
assert.sameValue(out.b, 2);
assert.sameValue(out.suffix, 3);