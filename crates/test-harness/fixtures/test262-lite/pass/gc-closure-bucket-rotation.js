/*---
description: gc stress with closure bucket rotation and object rebinding
---*/
function makeRotator(start) {
  let bucket = { total: start, node: { v: 1 } };
  return function (step) {
    bucket = {
      total: bucket.total + step,
      node: { v: bucket.node.v + 1 }
    };
    return bucket.total + bucket.node.v;
  };
}

let rotate = makeRotator(3);
let score = 0;
for (let i = 0; i < 28; i = i + 1) {
  score = score + rotate(1);
}

score;