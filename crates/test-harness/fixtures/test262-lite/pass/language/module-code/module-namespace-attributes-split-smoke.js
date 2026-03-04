/*---
flags: [module]
---*/

import * as ns from "./namespace-import-dep_FIXTURE.js" with
{ type: "json" };

if (ns.answer !== 42 || ns.default !== 7) {
  throw new Error("namespace import with split attributes clause should be accepted");
}
