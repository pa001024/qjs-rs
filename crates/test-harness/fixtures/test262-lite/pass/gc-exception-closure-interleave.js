/*---
description: gc stress interleaving try-catch-finally with closure captures
---*/
let tasks = [];
let token = { base: 1 };

for (let i = 0; i < 6; i = i + 1) {
  try {
    if (i % 2 === 0) {
      throw { code: i, marker: token.base + i };
    }
  } catch (err) {
    let box = { v: err.marker };
    tasks.push(function () {
      box.v = box.v + 1;
      return box.v;
    });
  } finally {
    token.base = token.base + 1;
  }
}

let total = 0;
for (let j = 0; j < tasks.length; j = j + 1) {
  total = total + tasks[j]();
}

total + token.base;