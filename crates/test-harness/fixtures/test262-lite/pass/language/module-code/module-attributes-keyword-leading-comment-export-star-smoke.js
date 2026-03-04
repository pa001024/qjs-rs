/*---
flags: [module]
---*/

export * from "./module-attributes-keyword-comment-dep_FIXTURE.js" /* gap */ with { mode: "strict" };
import { value } from "./module-attributes-keyword-comment-dep_FIXTURE.js";

if (value !== 42) {
  throw new Error("export-star should allow comment separators before attributes keyword");
}