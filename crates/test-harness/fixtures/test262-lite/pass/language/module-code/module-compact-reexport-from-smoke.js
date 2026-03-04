/*---
flags: [module]
---*/

import { value, answer } from "./module-compact-reexport-source_FIXTURE.js";

if (value !== 42 || answer !== 42) {
  throw new Error("compact re-export from syntax should resolve");
}
