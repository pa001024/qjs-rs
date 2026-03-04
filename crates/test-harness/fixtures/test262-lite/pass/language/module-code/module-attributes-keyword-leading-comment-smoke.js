/*---
flags: [module]
---*/

import { value } from "./module-attributes-keyword-comment-dep_FIXTURE.js" /* gap */ with { type: "json" };
export { value as answer } from "./module-attributes-keyword-comment-dep_FIXTURE.js" /* gap */ assert { type: "json" };

if (value !== 42) {
  throw new Error("attributes keyword should allow comment separators before keyword token");
}