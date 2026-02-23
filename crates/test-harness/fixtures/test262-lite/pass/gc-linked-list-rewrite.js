/*---
description: gc stress with linked list head rewrites and traversal
---*/
function prepend(head, value) {
  return { value: value, next: head };
}

let head = null;
for (let i = 0; i < 20; i = i + 1) {
  head = prepend(head, i);
}

function traverse(node) {
  let total = 0;
  while (node) {
    total = total + node.value;
    node = node.next;
  }
  return total;
}

let baseline = traverse(head);
for (let j = 0; j < 10; j = j + 1) {
  head = prepend(head.next, j + 100);
}

baseline + traverse(head);