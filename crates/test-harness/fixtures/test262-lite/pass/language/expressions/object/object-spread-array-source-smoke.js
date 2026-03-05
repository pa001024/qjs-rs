/*---
description: object spread supports array sources
---*/

var out = { ...[10, 20] };

assert.sameValue(out[0], 10);
assert.sameValue(out[1], 20);