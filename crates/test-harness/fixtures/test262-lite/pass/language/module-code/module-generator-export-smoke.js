/*---
flags: [module]
---*/

import { total, values } from "./module-generator-export-source_FIXTURE.js";

if (total !== 42 || typeof values !== "function") {
  throw new Error("generator export declaration should resolve");
}
