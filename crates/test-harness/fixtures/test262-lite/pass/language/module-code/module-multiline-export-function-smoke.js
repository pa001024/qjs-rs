/*---
flags: [module]
---*/

import { answer, build } from "./module-multiline-export-function-source_FIXTURE.js";

if (answer !== 42 || typeof build !== "function" || build() !== 42) {
  throw new Error("multiline export function declaration body should resolve");
}
