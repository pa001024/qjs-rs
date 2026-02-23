/*---
description: gc stress keeps with-scope object semantics stable
---*/
let obj = { x: 1 };
with (obj) {
  x = x + 2;
}
obj.x;
