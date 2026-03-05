/*---
description: object spread with string source and literal overwrite ordering
---*/

var out = { x: 1, ..."ab", x: 2 };

assert.sameValue(out.x, 2);
assert.sameValue(out[0], "a");
assert.sameValue(out[1], "b");