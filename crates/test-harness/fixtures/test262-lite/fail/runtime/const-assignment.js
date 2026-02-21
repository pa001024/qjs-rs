/*---
description: assignment to const should fail at runtime
negative:
  phase: runtime
  type: TypeError
---*/
const x = 1;
x = 2;
