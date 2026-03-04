/*---
flags: [module]
---*/

import { answer } from "./module-reexport-attributes-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("re-export attributes clause should be accepted");
}
