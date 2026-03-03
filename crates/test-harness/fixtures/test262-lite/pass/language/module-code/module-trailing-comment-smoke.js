/*---
flags: [module]
---*/

import { value } from "./module-trailing-comment-dep_FIXTURE.js" // trailing comment with from

if (value !== 42) {
  throw new Error("module trailing comment import should resolve");
}
