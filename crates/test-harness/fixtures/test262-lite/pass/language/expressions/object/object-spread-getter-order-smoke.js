/*---
description: object spread evaluates source properties in key order
---*/

var log = "";
var source = {
  get a() {
    log += "a";
    return 1;
  },
  get b() {
    log += "b";
    return 2;
  }
};

var out = { ...source };

assert.sameValue(log, "ab");
assert.sameValue(out.a, 1);
assert.sameValue(out.b, 2);