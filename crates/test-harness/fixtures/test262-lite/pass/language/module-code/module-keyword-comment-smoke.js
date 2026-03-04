/*---
flags: [module]
---*/

import { answer } from "./module-keyword-comment-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("keyword block-comment separators should be accepted");
}
