/*---
description: gc stress keeps cyclic objects reachable during call
---*/
let left = { name: "left" };
let right = { name: "right" };
left.peer = right;
right.peer = left;
function readCycle() {
  let view = left.peer;
  left = null;
  right = null;
  return view.peer.name;
}
readCycle();