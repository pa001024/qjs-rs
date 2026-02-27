/*---
description: Set smoke covers SameValueZero/live iteration and brand-check boundary errors
---*/
var nan = 0 / 0;
var set = new Set([nan, -0]);
assert.sameValue(set.has(nan), true);
assert.sameValue(set.has(+0), true);

var seen = [];
set.forEach(function (value) {
  seen.push(value);
  if (seen.length === 1) {
    set.add(1);
  }
});
assert.sameValue(seen.length, 3);
assert.sameValue(seen[2], 1);

assert.throws(TypeError, function () {
  Set();
});
assert.throws(TypeError, function () {
  Set.prototype.add.call({}, 1);
});
