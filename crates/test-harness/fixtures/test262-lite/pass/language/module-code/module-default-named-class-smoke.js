/*---
flags: [module]
---*/

import Counter, { answer } from "./module-default-named-class-source_FIXTURE.js";

if (typeof Counter !== "function" || answer !== 42 || Counter.base() !== 41) {
  throw new Error("default named class declaration should preserve local binding");
}
