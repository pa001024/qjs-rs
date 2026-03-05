/*---
description: object spread copies string index properties
---*/

var out = { ..."ab" };

assert.sameValue(out[0], "a");
assert.sameValue(out[1], "b");
assert.sameValue(Object.keys(out).join(","), "0,1");