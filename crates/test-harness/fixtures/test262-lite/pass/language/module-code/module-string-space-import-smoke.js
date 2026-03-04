/*---
flags: [module]
---*/

import { "kebab name" as kebabName } from "./module-string-space-export-source_FIXTURE.js";

if (kebabName !== 42) {
  throw new Error("string-named aliases with spaces should be accepted");
}
