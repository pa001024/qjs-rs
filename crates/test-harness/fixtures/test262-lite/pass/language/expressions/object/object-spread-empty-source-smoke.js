/*---
description: object spread from empty object keeps surrounding literals
---*/

var out = { before: 1, ...{}, after: 2 };

assert.sameValue(out.before, 1);
assert.sameValue(out.after, 2);