/*---
description: gc stress keeps closure-captured object reachable
---*/
let make = function () {
  let box = { value: 1 };
  return function (step) {
    box.value = box.value + step;
    return box.value;
  };
};
let fn = make();
fn(1) + fn(2);
