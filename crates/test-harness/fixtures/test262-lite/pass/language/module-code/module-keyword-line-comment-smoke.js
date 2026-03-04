/*---
flags: [module]
---*/

import { answer } from "./module-keyword-line-comment-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("keyword line-comment continuation should be accepted");
}
