/*---
flags: [module]
---*/

import { answer } from "./module-reexport-attributes-split-body-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("split re-export attributes clause body should be accepted");
}
