/*---
flags: [module]
---*/

import Counter, { answer } from "./module-multiline-default-class-source_FIXTURE.js";

if (typeof Counter !== "function" || Counter.value() !== 42 || answer !== 42) {
  throw new Error("multiline default class declaration body should resolve");
}
