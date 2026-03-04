/*---
flags: [module]
---*/

import { value }/* gap */from/* gap */"./module-import-attributes-dep_FIXTURE.js";

if (value !== 42) {
  throw new Error("comments around from keyword in import should be accepted");
}
