/*---
flags: [module]
description: object spread works in module code with imports
---*/

import { answer } from "../../module-code/namespace-import-dep_FIXTURE.js";

var out = { ...{ answer: answer }, tail: 1 };
if (out.answer !== 42 || out.tail !== 1) {
  throw new Error("module object spread should preserve imported values");
}