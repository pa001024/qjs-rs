/*---
description: repeated list rebuilds leave the old chains collectible during runtime GC
---*/
function buildList(depth) {
  let node = null;
  for (let i = 0; i < depth; i = i + 1) {
    node = { value: i, next: node };
  }
  return node;
}

let head = buildList(12);
for (let round = 0; round < 15; round = round + 1) {
  head = buildList(round + 5);
}
head.value;
