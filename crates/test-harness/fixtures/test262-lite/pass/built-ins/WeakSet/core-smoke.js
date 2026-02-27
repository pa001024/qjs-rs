/*---
description: WeakSet smoke covers object-value semantics and fail-fast constructor boundaries
---*/
var ws = new WeakSet();
var key = {};
ws.add(key);
assert.sameValue(ws.has(key), true);
assert.sameValue(ws.delete(key), true);
assert.sameValue(ws.has(key), false);

assert.throws(TypeError, function () {
  WeakSet();
});
assert.throws(TypeError, function () {
  ws.add(1);
});
assert.throws(TypeError, function () {
  ws.has(1);
});
assert.throws(TypeError, function () {
  ws.delete(1);
});

var pulls = 0;
var iterable = {
  [Symbol.iterator]: function () {
    return {
      next: function () {
        pulls = pulls + 1;
        if (pulls === 1) {
          return { value: {}, done: false };
        }
        if (pulls === 2) {
          return { value: 1, done: false };
        }
        return { value: undefined, done: true };
      }
    };
  }
};
assert.throws(TypeError, function () {
  new WeakSet(iterable);
});
assert.sameValue(pulls, 2);
