/*---
flags: [module]
---*/

import { "kebab name" as kebabName } from "./module-string-space-reexport-source_FIXTURE.js";

if (kebabName !== 42) {
  throw new Error("string-named re-export aliases with spaces should be accepted");
}
