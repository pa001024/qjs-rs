/*---
description: object baseline smoke
---*/

var target = { a: 1 };
Object.assign(target, { b: 2 });
assert.sameValue(target.a, 1);
assert.sameValue(target.b, 2);
assert.sameValue(Object.keys({ x: 1 }).length, 1);
