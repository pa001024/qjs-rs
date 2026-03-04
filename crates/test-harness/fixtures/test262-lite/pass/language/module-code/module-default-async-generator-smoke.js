/*---
flags: [module]
---*/

import generator, { genType } from "./module-default-async-generator-source_FIXTURE.js";

if (typeof generator !== "function" || genType !== "function") {
  throw new Error("default async generator declaration export should resolve");
}
