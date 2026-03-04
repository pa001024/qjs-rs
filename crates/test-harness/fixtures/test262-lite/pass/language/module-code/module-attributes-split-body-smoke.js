/*---
flags: [module]
---*/

import { value } from "./module-import-attributes-dep_FIXTURE.js" assert
{ type: "json" };

if (value !== 42) {
  throw new Error("split attributes clause body should be accepted");
}
