/*---
description: gc stress across try-catch-finally unwind
---*/
let state = { v: 0 };
function run() {
  try {
    state.v = 1;
    throw 0;
  } catch (e) {
    state.v = state.v + 1;
  } finally {
    state.v = state.v + 1;
  }
}
run();
state.v;
