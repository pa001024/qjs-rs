/*---
description: gc stress with loop-created frame closures retaining object snapshots
---*/
function makeFrame(tag, size) {
  let record = { tag: tag, payload: [] };
  for (let depth = 0; depth < size; depth = depth + 1) {
    record.payload.push({ depth: depth, label: tag + "-" + depth });
  }
  return function () {
    return record;
  };
}

let creators = [];
for (let round = 0; round < 5; round = round + 1) {
  creators.push(makeFrame("round" + round, 8));
}

let tally = 0;
for (let index = 0; index < creators.length; index = index + 1) {
  let record = creators[index]();
  tally = tally + record.payload.length + record.tag.length;
}

tally;