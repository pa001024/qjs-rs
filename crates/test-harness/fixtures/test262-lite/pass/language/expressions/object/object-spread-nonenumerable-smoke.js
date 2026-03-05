/*---
description: object spread skips non-enumerable properties
---*/

var source = {};
Object.defineProperty(source, "hidden", {
  value: 1,
  enumerable: false,
  configurable: true,
  writable: true
});
source.visible = 2;

var out = { ...source };
assert.sameValue(out.hidden, undefined);
assert.sameValue(out.visible, 2);