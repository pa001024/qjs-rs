/*---
description: object spread tolerates multiple empty/nullish sources
---*/

var out = { ...{}, ...null, ...undefined, ...{}, value: 1 };

assert.sameValue(out.value, 1);
assert.sameValue(Object.keys(out).length, 1);