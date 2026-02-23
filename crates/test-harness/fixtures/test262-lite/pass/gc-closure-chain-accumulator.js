/*---
description: gc stress keeps linked closure chain reachable across repeated reads
---*/
function makeAccumulator(depth) {
  let head = { value: depth, next: null };
  for (let i = depth - 1; i >= 0; i = i - 1) {
    head = { value: i, next: head };
  }
  return function (offset) {
    let cursor = head;
    let total = 0;
    while (cursor) {
      total = total + cursor.value;
      cursor = cursor.next;
    }
    return total + offset;
  };
}

let chain = makeAccumulator(8);
let sum = 0;
for (let iter = 0; iter < 4; iter = iter + 1) {
  sum = sum + chain(iter);
}

sum;