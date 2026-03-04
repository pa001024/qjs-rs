/*---
flags: [module]
---*/

import {
  value,
  extra,
  first,
  third,
  right,
} from "./module-destructuring-export-source_FIXTURE.js";

if (value !== 40 || extra !== 2 || first !== 1 || third !== 3 || right !== 42) {
  throw new Error("destructuring export declarations should resolve");
}
