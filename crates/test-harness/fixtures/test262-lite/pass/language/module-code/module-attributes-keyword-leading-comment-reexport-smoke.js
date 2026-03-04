/*---
flags: [module]
---*/

export { value as answer } from "./module-attributes-keyword-comment-dep_FIXTURE.js" /* gap */ assert { type: "json" };
import { answer } from "./module-attributes-keyword-leading-comment-reexport-source_FIXTURE.js";

if (answer !== 42) {
  throw new Error("named re-export should allow comment separators before attributes keyword");
}