/*---
flags: [module]
---*/

import { value, bridge } from "./export-star-source_FIXTURE.js";
import fallback from "./export-star-source_FIXTURE.js";

if (value !== 42) {
  throw new Error("export * should forward named export");
}
if (bridge !== 1) {
  throw new Error("source module direct named export mismatch");
}
if (typeof fallback !== "undefined") {
  throw new Error("export * should not forward default export");
}
