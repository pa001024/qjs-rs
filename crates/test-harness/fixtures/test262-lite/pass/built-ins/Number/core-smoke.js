/*---
description: number baseline smoke
---*/

assert.sameValue(Number('42'), 42);
assert.sameValue(Number.isFinite(1), true);
assert.sameValue(Number.isFinite('1'), false);
assert.sameValue(Number.isInteger(-0), true);
assert.sameValue(Number.isSafeInteger(9007199254740991), true);
assert.sameValue(Number.isSafeInteger(9007199254740992), false);
