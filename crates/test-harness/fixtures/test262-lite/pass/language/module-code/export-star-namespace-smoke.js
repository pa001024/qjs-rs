/*---
flags: [module]
---*/

import { ns } from "./export-star-namespace-source_FIXTURE.js";

if (typeof ns !== "object" || ns === null) {
  throw new Error("export * as ns should expose namespace object");
}
if (ns.value !== 42) {
  throw new Error("namespace should expose named export");
}
if (ns.default !== 7) {
  throw new Error("namespace should expose default export");
}
