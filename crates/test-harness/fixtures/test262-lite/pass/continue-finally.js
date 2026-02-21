/*---
description: continue should run finally in loop
---*/
let x = 0;
for (let i = 0; i < 3; i = i + 1) {
  try {
    if (i == 1) continue;
    x = x + 1;
  } finally {
    x = x + 10;
  }
}
x;
