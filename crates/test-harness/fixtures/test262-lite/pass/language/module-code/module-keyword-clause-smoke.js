/*---
flags: [module]
---*/

import { if as condition } from "./module-keyword-clause-source_FIXTURE.js";

if (condition !== 42) {
  throw new Error("keyword alias names in module clauses should resolve");
}
