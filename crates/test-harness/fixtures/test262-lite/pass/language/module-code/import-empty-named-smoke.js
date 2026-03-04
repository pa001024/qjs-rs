/*---
flags: [module]
---*/

import {} from "./import-empty-named-dep_FIXTURE.js";

var sentinel = 42;
if (sentinel !== 42) {
  throw new Error("empty named import should not alter execution");
}
