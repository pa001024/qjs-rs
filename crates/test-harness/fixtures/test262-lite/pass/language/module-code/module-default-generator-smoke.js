/*---
flags: [module]
---*/

import generator, { total } from "./module-default-generator-source_FIXTURE.js";

if (typeof generator !== "function" || total !== 42) {
  throw new Error("default generator declaration export should resolve");
}
