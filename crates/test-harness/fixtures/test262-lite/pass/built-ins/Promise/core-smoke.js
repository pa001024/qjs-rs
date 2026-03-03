/*---
description: promise static baseline smoke
---*/

assert.sameValue(typeof Promise.resolve, "function");
assert.sameValue(typeof Promise.reject, "function");
assert.sameValue(typeof Promise.all, "function");
assert.sameValue(typeof Promise.any, "function");
assert.sameValue(typeof Promise.race, "function");
assert.sameValue(typeof Promise.allSettled, "function");

var resolved = Promise.resolve(1);
assert.sameValue(typeof resolved.then, "function");
assert.sameValue(Promise.resolve(resolved), resolved);

var rejected = Promise.reject(1);
assert.sameValue(typeof rejected.catch, "function");

var nonIterableThrown = false;
try {
  Promise.all(null);
} catch (err) {
  nonIterableThrown = err instanceof TypeError;
}
assert.sameValue(nonIterableThrown, true);

assert.sameValue(typeof Promise.all([]).then, "function");
assert.sameValue(typeof Promise.any([]).then, "function");
assert.sameValue(typeof Promise.race([]).then, "function");
assert.sameValue(typeof Promise.allSettled([]).then, "function");
