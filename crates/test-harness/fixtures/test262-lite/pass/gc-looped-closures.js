/*---
description: gc stress with loop-created closures and object captures
---*/
let list = [];
for (let i = 0; i < 4; i = i + 1) {
  let node = { value: i };
  list.push(function () {
    node.value = node.value + 1;
    return node.value;
  });
}
let sum = 0;
for (let j = 0; j < list.length; j = j + 1) {
  sum = sum + list[j]();
}
sum;