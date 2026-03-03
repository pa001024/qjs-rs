/*---
flags: [module]
---*/

import {
  value,
  extra as bonus,
}
from
  "./module-multiline-dep_FIXTURE.js";

if (value + bonus !== 42) {
  throw new Error("multiline import should resolve with split from-clause");
}
