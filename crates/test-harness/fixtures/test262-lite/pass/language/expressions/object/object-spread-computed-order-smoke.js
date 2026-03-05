/*---
description: object spread and computed keys preserve evaluation order
---*/

var log = "";
function source() {
  log += "s";
  return { x: 1 };
}

var out = { ...source(), [(log += "k", "y")]: 2 };
assert.sameValue(log, "sk");
assert.sameValue(out.x, 1);
assert.sameValue(out.y, 2);