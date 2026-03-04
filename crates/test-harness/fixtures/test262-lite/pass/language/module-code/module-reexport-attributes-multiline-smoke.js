/*---
flags: [module]
---*/

import { answer } from "./module-reexport-attributes-multiline-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("multiline re-export attributes clause should be accepted");
}
