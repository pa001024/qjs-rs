/*---
flags: [module]
---*/

import { answer } from "./module-from-comment-reexport-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("comments around from keyword in re-export should be accepted");
}
