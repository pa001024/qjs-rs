/*---
flags: [module]
---*/

import fallback/* gap */,/* gap */* as ns from "./module-mixed-import-comment-comma-dep_FIXTURE.js";

if (fallback + ns.value !== 42) {
  throw new Error("default+namespace import should allow comment separators around comma");
}