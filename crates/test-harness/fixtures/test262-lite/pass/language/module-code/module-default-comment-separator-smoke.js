/*---
flags: [module]
---*/

import named, { namedType } from "./module-default-comment-separator-source_FIXTURE.js";

if (typeof named !== "function" || namedType !== "function") {
  throw new Error("default declaration with comment separator should be accepted");
}
