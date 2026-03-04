/*---
description: object spread over Number wrapper preserves explicit literal fields
---*/

var out = { ...new Number(7), tail: 1 };
assert.sameValue(out.tail, 1);