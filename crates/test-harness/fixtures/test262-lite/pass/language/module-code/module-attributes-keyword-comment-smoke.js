/*---
flags: [module]
---*/

import { value } from "./module-attributes-keyword-comment-dep_FIXTURE.js" with/* gap */{ type: "json" };
export { value as answer } from "./module-attributes-keyword-comment-dep_FIXTURE.js" assert/* gap */{ type: "json" };
export * from "./module-attributes-keyword-comment-dep_FIXTURE.js" with/* gap */{ mode: "strict" };

if (value !== 42) {
  throw new Error("attributes keyword should allow comment separators before clause body");
}