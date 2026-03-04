/*---
description: object spread ignores non-object primitive sources
---*/

var out = { ...1, ...true, ...false, after: 7 };

assert.sameValue(out.after, 7);
assert.sameValue(Object.keys(out).length, 1);