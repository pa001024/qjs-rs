/*---
description: gc stress with deep object chain traversed by closure
---*/
function makeChain(depth) {
  let head = { value: 0 };
  let current = head;
  for (let i = 1; i < depth; i = i + 1) {
    current.next = { value: i };
    current = current.next;
  }
  return head;
}

let chain = makeChain(24);
function readChain() {
  let sum = 0;
  let node = chain;
  while (node) {
    sum = sum + node.value;
    node = node.next;
  }
  return sum;
}

let probe = 0;
for (let k = 0; k < 10; k = k + 1) {
  let temp = { x: k, y: { z: k + 1 } };
  probe = probe + temp.y.z;
}

readChain() + probe;