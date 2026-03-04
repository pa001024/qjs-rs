/*---
flags: [module]
description: module object spread with imported bindings
---*/

import fallback, { answer } from "../../module-code/namespace-import-dep_FIXTURE.js";

var out = { ...{ fallback: fallback }, ...{ answer: answer }, tail: 1 };
if (out.fallback !== 7 || out.answer !== 42 || out.tail !== 1) {
  throw new Error("module object spread should preserve imported bindings");
}