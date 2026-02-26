/*---
description: NativeErrors constructors expose dedicated prototype chains
---*/
var constructors = [
  TypeError,
  ReferenceError,
  SyntaxError,
  RangeError,
  EvalError,
  URIError
];

for (var i = 0; i < constructors.length; i++) {
  var C = constructors[i];
  assert.notSameValue(C.prototype, Error.prototype);
  assert.sameValue(Object.getPrototypeOf(C.prototype), Error.prototype);
  assert.sameValue(C.prototype.constructor, C);
}
