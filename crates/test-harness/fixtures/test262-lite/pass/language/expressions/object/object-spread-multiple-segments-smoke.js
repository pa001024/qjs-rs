/*---
description: object spread can appear multiple times in one literal
---*/

var out = { ...{ a: 1 }, mid: 2, ...{ b: 3 }, tail: 4 };

assert.sameValue(out.a, 1);
assert.sameValue(out.mid, 2);
assert.sameValue(out.b, 3);
assert.sameValue(out.tail, 4);