/*---
flags: [module]
description: module code supports object spread expressions
---*/

import { answer } from "../../module-code/namespace-import-dep_FIXTURE.js";

var out = { base: 1, ...{ answer: answer }, tail: 2 };
if (out.base !== 1 || out.answer !== 42 || out.tail !== 2) {
  throw new Error("module object spread should evaluate");
}
