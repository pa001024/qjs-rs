/*---
description: gc stress across nested try-catch-finally with captured records
---*/
let tasks = [];

for (let i = 0; i < 8; i = i + 1) {
  try {
    try {
      if (i % 2 === 0) {
        throw { code: i, value: i + 100 };
      }
    } catch (inner) {
      let record = { x: inner.value, y: i };
      tasks.push(function () {
        record.x = record.x + 1;
        return record.x + record.y;
      });
    } finally {
      let cleanup = { z: i + 1 };
      cleanup.z = cleanup.z + 1;
    }
  } catch (outer) {
    throw outer;
  }
}

let out = 0;
for (let j = 0; j < tasks.length; j = j + 1) {
  out = out + tasks[j]();
}

out;