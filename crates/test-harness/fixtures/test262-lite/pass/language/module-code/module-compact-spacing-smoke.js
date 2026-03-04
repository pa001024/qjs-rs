/*---
flags: [module]
---*/

import{ value }from"./module-compact-spacing-dep_FIXTURE.js";

if (value !== 42) {
  throw new Error("compact spacing module import should resolve");
}
