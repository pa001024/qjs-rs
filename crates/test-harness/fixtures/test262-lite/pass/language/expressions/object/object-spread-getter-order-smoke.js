/*---
description: object spread evaluates source expressions in left-to-right order
---*/

var log = "";
function first() {
  log += "a";
  return { a: 1 };
}
function second() {
  log += "b";
  return { b: 2 };
}

var out = { ...first(), ...second() };

assert.sameValue(log, "ab");
assert.sameValue(out.a, 1);
assert.sameValue(out.b, 2);