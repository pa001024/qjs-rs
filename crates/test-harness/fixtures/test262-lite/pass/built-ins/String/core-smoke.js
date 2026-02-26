/*---
description: string baseline smoke
---*/

assert.sameValue(String('abc'), 'abc');
assert.sameValue(String.fromCharCode(65, 66, 67), 'ABC');

var threw = false;
try {
  String.fromCharCode({ valueOf: function () { throw 'boom'; } });
} catch (err) {
  threw = err === 'boom';
}
assert.sameValue(threw, true);
