/*---
description: JSON.parse supports reviver transforms and malformed input SyntaxError
---*/
var source = '{"keep":1,"drop":2,"nested":{"value":3},"arr":[1,2,3]}';
var parsed = JSON.parse(source, function (key, value) {
  if (key === 'drop') {
    return undefined;
  }
  if (key === 'value') {
    return value * 10;
  }
  if (key === '1') {
    return value + 40;
  }
  return value;
});

assert.sameValue(parsed.keep, 1);
assert.sameValue('drop' in parsed, false);
assert.sameValue(parsed.nested.value, 30);
assert.sameValue(parsed.arr[1], 42);
assert.sameValue(parsed.arr.length, 3);
assert.throws(SyntaxError, function () {
  JSON.parse('{"x":');
});
