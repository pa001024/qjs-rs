/*---
flags: [module]
---*/

import { "kebab-name" as kebabName } from "./module-string-named-clause-source_FIXTURE.js";

if (kebabName !== 42) {
  throw new Error("string-named module clause aliases should resolve");
}
