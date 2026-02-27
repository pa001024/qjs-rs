/*---
description: WeakMap smoke covers object-key semantics and fail-fast constructor boundaries
---*/
var wm = new WeakMap();
var key = {};
wm.set(key, 42);
assert.sameValue(wm.get(key), 42);
assert.sameValue(wm.has(key), true);
assert.sameValue(wm.delete(key), true);
assert.sameValue(wm.has(key), false);

assert.throws(TypeError, function () {
  WeakMap();
});
assert.throws(TypeError, function () {
  wm.set(1, 1);
});
assert.throws(TypeError, function () {
  wm.get(1);
});
assert.throws(TypeError, function () {
  wm.has(1);
});
assert.throws(TypeError, function () {
  wm.delete(1);
});

var pulls = 0;
var iterable = {
  [Symbol.iterator]: function () {
    return {
      next: function () {
        pulls = pulls + 1;
        if (pulls === 1) {
          return { value: [{}, 1], done: false };
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
  new WeakMap(iterable);
});
assert.sameValue(pulls, 2);
