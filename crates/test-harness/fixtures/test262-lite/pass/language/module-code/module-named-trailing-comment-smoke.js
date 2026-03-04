/*---
flags: [module]
---*/

const value = 42;
export { value/* gap */ };

if (value !== 42) {
  throw new Error("named export entries should allow trailing comments");
}