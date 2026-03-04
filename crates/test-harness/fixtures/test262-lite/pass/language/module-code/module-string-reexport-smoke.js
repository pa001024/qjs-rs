/*---
flags: [module]
---*/

import { "kebab-name" as kebabName } from "./module-string-reexport-source_FIXTURE.js";

if (kebabName !== 42) {
  throw new Error("string-named re-export clause should resolve");
}
