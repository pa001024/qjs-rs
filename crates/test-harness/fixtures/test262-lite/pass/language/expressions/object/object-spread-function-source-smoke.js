/*---
description: object spread copies function object own properties
---*/

function source() {}
source.answer = 42;

var out = { ...source };
assert.sameValue(out.answer, 42);