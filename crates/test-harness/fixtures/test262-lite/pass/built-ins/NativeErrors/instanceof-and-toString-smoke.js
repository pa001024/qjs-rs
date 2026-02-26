/*---
description: NativeErrors instances satisfy subclass/Error instanceof and toString behavior
---*/
var ref = new ReferenceError("ref");
assert(ref instanceof ReferenceError);
assert(ref instanceof Error);
assert.sameValue(ref.toString(), "ReferenceError: ref");

var uri = new URIError("uri");
assert(uri instanceof URIError);
assert(uri instanceof Error);
assert.sameValue(uri.toString(), "URIError: uri");

var threw = false;
try {
  Error.prototype.toString.call(1);
} catch (e) {
  threw = e instanceof TypeError;
}
assert.sameValue(threw, true);
