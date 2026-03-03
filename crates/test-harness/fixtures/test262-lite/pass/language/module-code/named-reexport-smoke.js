/*---
flags: [module]
---*/

import { answer, fallback } from "./named-reexport-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("named re-export should forward named binding");
}
if (fallback !== 7) {
  throw new Error("named re-export should forward default binding alias");
}
