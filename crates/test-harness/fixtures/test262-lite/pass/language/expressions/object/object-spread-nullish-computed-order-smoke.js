/*---
description: computed key still evaluates between nullish object spreads
---*/

var log = "";
function key() {
  log += "k";
  return "x";
}

var out = { ...null, [key()]: 1, ...undefined };
assert.sameValue(log, "k");
assert.sameValue(out.x, 1);