/*---
description: object spread over Date source keeps surrounding literal fields
---*/

var out = { before: 1, ...new Date(0), after: 2 };
assert.sameValue(out.before, 1);
assert.sameValue(out.after, 2);