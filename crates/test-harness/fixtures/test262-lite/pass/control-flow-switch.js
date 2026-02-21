/*---
description: switch fallthrough baseline
---*/
let y = 0;
switch (1) {
  case 1:
    y = y + 1;
  case 2:
    y = y + 2;
    break;
  default:
    y = y + 4;
}
y;
