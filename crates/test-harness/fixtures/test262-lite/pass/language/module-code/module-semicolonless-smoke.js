/*---
flags: [module]
---*/

import { value } from "./module-semicolonless-dep_FIXTURE.js"

if (value !== 42) {
  throw new Error("semicolonless module import should resolve");
}
