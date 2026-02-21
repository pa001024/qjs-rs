/*---
description: finally return overrides try return
---*/
function f() {
  try {
    return 1;
  } finally {
    return 2;
  }
}
f();
