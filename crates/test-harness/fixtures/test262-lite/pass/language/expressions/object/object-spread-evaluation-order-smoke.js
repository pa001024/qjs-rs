/*---
description: object spread source expressions evaluate left-to-right
---*/

var callCount = 0;
function next() {
  callCount += 1;
  if (callCount === 1) {
    return { left: 40 };
  }
  return { right: 2 };
}

var out = { ...next(), ...next() };
assert.sameValue(callCount, 2);
assert.sameValue(out.left, 40);
assert.sameValue(out.right, 2);