/*---
description: function baseline smoke
---*/

var add = Function('a', 'b', 'return a + b;');
assert.sameValue(add(20, 22), 42);
assert.sameValue(Function.length, 1);
assert.throws(SyntaxError, function () {
  Function('[');
});
