/*---
description: object spread does not copy prototype properties
---*/

var proto = { inherited: 1 };
var source = Object.create(proto);
source.own = 2;

var out = { ...source };
assert.sameValue(out.own, 2);
assert.sameValue(out.inherited, undefined);