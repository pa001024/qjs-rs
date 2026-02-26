/*---
description: JSON.stringify throws TypeError for cyclic structures
---*/
var cycle = {};
cycle.self = cycle;

assert.throws(TypeError, function () {
  JSON.stringify(cycle);
});
