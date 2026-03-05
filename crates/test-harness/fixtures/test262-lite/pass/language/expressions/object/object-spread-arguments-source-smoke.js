/*---
description: object spread copies indexed entries from arguments objects
---*/

(function(a, b, c) {
  var out = { ...arguments };
  assert.sameValue(out[0], a);
  assert.sameValue(out[1], b);
  assert.sameValue(out[2], c);
})(1, 2, 3);