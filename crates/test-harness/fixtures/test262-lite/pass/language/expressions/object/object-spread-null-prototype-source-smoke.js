/*---
description: object spread works with null-prototype sources
---*/

var source = Object.create(null);
source.answer = 42;

var out = { ...source };
assert.sameValue(out.answer, 42);
assert.sameValue(Object.getPrototypeOf(source), null);