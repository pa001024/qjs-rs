/*---
flags: [module]
---*/

import fallback/* gap */,/* gap */{ value as named } from "./module-mixed-import-comment-comma-dep_FIXTURE.js";

if (fallback + named !== 42) {
  throw new Error("default+named import should allow comment separators around comma");
}