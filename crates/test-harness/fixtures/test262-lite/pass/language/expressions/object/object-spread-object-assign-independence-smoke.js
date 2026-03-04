/*---
description: object spread remains independent from global Object.assign override
---*/

Object.assign = function() {
  throw new Error("Object.assign should not be used by object spread");
};

var out = { ...{ a: 1 } };
assert.sameValue(out.a, 1);