/*---
flags: [module]
---*/

import "./module-attributes-keyword-comment-dep_FIXTURE.js" /* gap */ assert { type: "json" };

export const answer = 42;
if (answer !== 42) {
  throw new Error("bare import should allow comment separators before attributes keyword");
}