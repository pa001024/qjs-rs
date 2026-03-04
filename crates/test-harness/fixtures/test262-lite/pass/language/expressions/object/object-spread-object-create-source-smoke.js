/*---
description: object spread copies only own enumerable fields from Object.create sources
---*/

var source = Object.create({ inherited: 1 });
source.left = 40;
source.right = 2;

var out = { ...source };
assert.sameValue(out.left, 40);
assert.sameValue(out.right, 2);
assert.sameValue(out.inherited, undefined);