/*---
flags: [module]
---*/

import *
as
ns from "./namespace-import-dep_FIXTURE.js";

if (ns.answer !== 42 || ns.default !== 7) {
  throw new Error("namespace import across linebreaks should be accepted");
}
