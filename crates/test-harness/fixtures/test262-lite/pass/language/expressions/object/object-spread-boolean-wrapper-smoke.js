/*---
description: object spread over Boolean wrapper preserves explicit literal fields
---*/

var out = { ...new Boolean(true), tail: 1 };
assert.sameValue(out.tail, 1);