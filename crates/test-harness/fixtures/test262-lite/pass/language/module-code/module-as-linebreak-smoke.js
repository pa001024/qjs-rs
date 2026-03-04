/*---
flags: [module]
---*/

import {
  value
  as
  alias,
} from "./module-as-linebreak-source_FIXTURE.js";

if (alias !== 42) {
  throw new Error("linebreak as-alias clauses should be accepted");
}
