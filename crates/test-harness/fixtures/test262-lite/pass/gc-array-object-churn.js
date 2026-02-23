/*---
description: gc stress with array object churn and retained slot
---*/
function makeChunk(seed) {
  let arr = [];
  for (let i = 0; i < 6; i = i + 1) {
    arr.push({ value: seed + i });
  }
  return arr;
}

let slots = [makeChunk(1), makeChunk(10), makeChunk(20)];
let keep = function () {
  return slots[1][2].value;
};

for (let round = 0; round < 18; round = round + 1) {
  slots[round % 3] = makeChunk(round * 3);
}

keep() + slots[2][0].value;