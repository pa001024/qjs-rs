/*---
description: object spread keeps shallow reference identity for property values
---*/

var inner = { value: 1 };
var out = { ...{ inner: inner } };
inner.value = 2;

assert.sameValue(out.inner.value, 2);