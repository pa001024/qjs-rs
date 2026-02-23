/*---
description: runtime GC reclaims transient arrays built inside repeated batches
---*/
function buildBatch(seed) {
  let batch = [];
  for (let i = 0; i < 12; i = i + 1) {
    batch.push({
      value: seed + i,
      nested: [{ value: seed + i * 10 }, { value: seed + i * 10 + 1 }],
    });
  }
  return batch;
}

let total = 0;
for (let iter = 0; iter < 18; iter = iter + 1) {
  total = total + buildBatch(iter * 3).length;
}
total;
