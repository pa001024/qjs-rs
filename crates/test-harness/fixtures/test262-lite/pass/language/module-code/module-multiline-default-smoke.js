/*---
flags: [module]
---*/

import value from "./module-multiline-default-source_FIXTURE.js";

if (value !== 42) {
  throw new Error("multiline default export expression should resolve");
}
