/*---
description: boolean baseline smoke
---*/

assert.sameValue(Boolean(0), false);
assert.sameValue(Boolean(1), true);
assert.sameValue((new Boolean(true)).valueOf(), true);
