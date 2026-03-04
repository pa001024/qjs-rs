/*---
description: object spread ignores null and undefined sources
---*/

var out = { before: 1, ...null, middle: 2, ...undefined, after: 3 };

assert.sameValue(out.before, 1);
assert.sameValue(out.middle, 2);
assert.sameValue(out.after, 3);