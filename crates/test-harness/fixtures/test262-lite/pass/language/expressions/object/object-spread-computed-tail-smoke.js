/*---
description: object spread with computed property tail
---*/

var key = "answer";
var out = { ...{ left: 40 }, [key]: 2 };

assert.sameValue(out.left, 40);
assert.sameValue(out.answer, 2);