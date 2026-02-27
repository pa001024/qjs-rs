/*---
description: Map smoke covers SameValueZero/live iteration and brand-check boundary errors
---*/
var nan = 0 / 0;
var map = new Map();
map.set(nan, 'nan');
map.set(-0, 'zero');
assert.sameValue(map.has(nan), true);
assert.sameValue(map.get(nan), 'nan');
assert.sameValue(map.get(+0), 'zero');

var seen = [];
var ordered = new Map([
  ['a', 1],
  ['b', 2]
]);
ordered.set('a', 3);
ordered.delete('a');
ordered.set('a', 4);
ordered.forEach(function (value, key) {
  seen.push(key + ':' + value);
  if (key === 'b') {
    ordered.set('c', 5);
  }
});
assert.sameValue(seen.length, 3);
assert.sameValue(seen[0], 'b:2');
assert.sameValue(seen[1], 'a:4');
assert.sameValue(seen[2], 'c:5');

assert.throws(TypeError, function () {
  Map();
});
assert.throws(TypeError, function () {
  Map.prototype.get.call({}, 'x');
});
