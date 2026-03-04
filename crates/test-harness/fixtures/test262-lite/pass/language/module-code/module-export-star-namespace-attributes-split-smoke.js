/*---
flags: [module]
---*/

import { ns } from "./module-export-star-namespace-attributes-split-source_FIXTURE.js";

if (ns.value !== 42 || ns.default !== 7) {
  throw new Error("export-star namespace with split attributes clause should be accepted");
}
