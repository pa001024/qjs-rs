/*---
description: Error constructor defaults and Error.prototype.toString smoke
---*/
var base = Error();
assert.sameValue(base.name, "Error");
assert.sameValue(base.message, "");
assert.sameValue(base.toString(), "Error");

var explicitUndefined = Error(undefined);
assert.sameValue(explicitUndefined.message, "");
assert.sameValue(explicitUndefined.toString(), "Error");
