/*---
flags: [module]
---*/

import * as/* gap */ns from "./namespace-import-dep_FIXTURE.js";

if (ns.answer !== 42 || ns.default !== 7) {
  throw new Error("namespace import with comment after as should be accepted");
}
