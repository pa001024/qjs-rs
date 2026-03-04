/*---
flags: [module]
---*/

import * as ns from "./namespace-import-dep_FIXTURE.js" /* gap */ with { type: "json" };

if (ns.answer !== 42 || ns.default !== 7) {
  throw new Error("namespace import should allow comment separators before attributes keyword");
}