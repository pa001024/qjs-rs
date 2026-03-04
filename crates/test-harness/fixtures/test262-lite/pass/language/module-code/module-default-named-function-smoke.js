/*---
flags: [module]
---*/

import named, { answer } from "./module-default-named-function-source_FIXTURE.js";

if (typeof named !== "function" || answer !== 42 || named() !== 41) {
  throw new Error("default named function declaration should preserve local binding");
}
