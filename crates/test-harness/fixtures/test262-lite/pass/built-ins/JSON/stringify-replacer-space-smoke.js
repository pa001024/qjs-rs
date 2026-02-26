/*---
description: JSON.stringify supports replacer and space baseline behavior
---*/
var input = {
  keep: 1,
  drop: 2,
  nested: { value: 3 },
  arr: [1, undefined, function () {}, 4]
};
var pretty = JSON.stringify(
  input,
  function (key, value) {
    if (key === 'drop') {
      return undefined;
    }
    if (key === 'value') {
      return value + 7;
    }
    return value;
  },
  2
);

assert.sameValue(pretty.indexOf('"drop"'), -1);
assert.notSameValue(pretty.indexOf('\n  "nested": {\n    "value": 10\n  }'), -1);
assert.notSameValue(
  pretty.indexOf('"arr": [\n    1,\n    null,\n    null,\n    4\n  ]'),
  -1
);

var listed = JSON.stringify({ a: 1, b: 2, c: 3 }, ['c', 'a'], '..........++++');
assert.sameValue(listed, '{\n.........."c": 3,\n.........."a": 1\n}');
assert.sameValue(JSON.stringify([1, undefined, function () {}, 4]), '[1,null,null,4]');
