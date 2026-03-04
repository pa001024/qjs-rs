/*---
flags: [module]
---*/

import
{ value } from "./module-import-attributes-dep_FIXTURE.js";

if (value !== 42) {
  throw new Error("keyword-only linebreak module declarations should be accepted");
}
