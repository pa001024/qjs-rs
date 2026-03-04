/*---
flags: [module]
---*/

import { ns } from "./module-export-star-namespace-as-comment-source_FIXTURE.js";

if (ns.value !== 42 || ns.default !== 7) {
  throw new Error("export-star namespace with comment after as should be accepted");
}
