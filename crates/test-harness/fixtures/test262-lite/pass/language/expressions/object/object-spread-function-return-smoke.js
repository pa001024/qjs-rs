/*---
description: object spread works in function return literals
---*/

function build(extra) {
  return { base: 40, ...extra, tail: 2 };
}

var out = build({ mid: 0 });
assert.sameValue(out.base, 40);
assert.sameValue(out.mid, 0);
assert.sameValue(out.tail, 2);