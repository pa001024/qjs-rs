/*---
flags: [module]
---*/

import { value }   from   "./import-from-extra-spacing-dep_FIXTURE.js";

if (value !== 42) {
  throw new Error("import from with extra spacing should resolve");
}
