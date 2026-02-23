/*---
description: gc stress across nested closure chains
---*/
function buildChain(seed) {
  let root = { total: seed };
  return function (delta) {
    let local = { value: delta };
    return function (extra) {
      local.value = local.value + extra;
      root.total = root.total + local.value;
      return root.total;
    };
  };
}
let make = buildChain(1);
let first = make(2);
let second = make(3);
first(4) + second(5);