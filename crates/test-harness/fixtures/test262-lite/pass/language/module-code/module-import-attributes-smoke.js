/*---
flags: [module]
---*/

import { value } from "./module-import-attributes-dep_FIXTURE.js" with { type: "json" };

if (value !== 42) {
  throw new Error("import attributes clause should be accepted");
}
