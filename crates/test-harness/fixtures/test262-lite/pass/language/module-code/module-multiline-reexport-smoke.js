/*---
flags: [module]
---*/

import {
  answer,
  fallback,
} from "./module-multiline-reexport-source_FIXTURE.js";

if (answer !== 40 || fallback !== 7) {
  throw new Error("multiline named re-export should resolve");
}
