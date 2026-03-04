/*---
flags: [module]
---*/

import fallback/* gap */from "./namespace-import-dep_FIXTURE.js";
import fallbackNamed, { answer as named }/* gap */from "./namespace-import-dep_FIXTURE.js";
import fallbackNs, * as ns/* gap */from "./namespace-import-dep_FIXTURE.js";

if (fallback + fallbackNamed + named + fallbackNs + ns.answer !== 105) {
  throw new Error("default import forms should allow comments before from keyword");
}