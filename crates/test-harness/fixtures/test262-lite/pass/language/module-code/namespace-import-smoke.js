/*---
flags: [module]
---*/

import * as ns from "./namespace-import-dep_FIXTURE.js";

if (typeof ns !== "object" || ns === null) {
  throw new Error("namespace import should be an object");
}
if (ns.answer !== 42) {
  throw new Error("namespace named export mismatch");
}
if (ns.default !== 7) {
  throw new Error("namespace default export mismatch");
}
