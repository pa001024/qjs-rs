/*---
flags: [module]
---*/

import { value/* gap */as/* gap */alias } from "./module-import-attributes-dep_FIXTURE.js";

if (alias !== 42) {
  throw new Error("named alias with comments around as should be accepted");
}
