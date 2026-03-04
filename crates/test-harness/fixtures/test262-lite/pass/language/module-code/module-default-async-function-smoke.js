/*---
flags: [module]
---*/

import named, { namedType } from "./module-default-async-function-source_FIXTURE.js";

if (typeof named !== "function" || namedType !== "function") {
  throw new Error("default async function declaration export should resolve");
}
