/*---
description: gc stress with array ring buffer replacement and closure reads
---*/
function makeChunk(seed) {
  return [
    { value: seed },
    { value: seed + 1 },
    { value: seed + 2 },
    { value: seed + 3 }
  ];
}

let ring = [makeChunk(1), makeChunk(10), makeChunk(20), makeChunk(30)];
function probe(index) {
  let chunk = ring[index % ring.length];
  return chunk[0].value + chunk[3].value;
}

let sum = 0;
for (let i = 0; i < 24; i = i + 1) {
  ring[i % ring.length] = makeChunk(i * 2);
  sum = sum + probe(i);
}

sum;