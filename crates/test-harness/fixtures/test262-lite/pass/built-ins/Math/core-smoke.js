/*---
description: math baseline smoke
---*/

assert.sameValue(Math.clz32(1), 31);
assert.sameValue(Math.hypot(3, 4), 5);
assert.sameValue(Math.log2(8), 3);
assert.sameValue(Math.log10(1000), 3);
assert.sameValue(1 / Math.sign(-0), -Infinity);
