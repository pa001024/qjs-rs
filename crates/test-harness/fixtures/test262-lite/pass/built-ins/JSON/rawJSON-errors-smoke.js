/*---
description: JSON.rawJSON rejects invalid inputs and invalid JSON texts
---*/
function assertRawJSONSyntaxError(value) {
  assert.throws(SyntaxError, function () {
    JSON.rawJSON(value);
  });
}

assert.throws(TypeError, function () {
  JSON.rawJSON(Symbol("1"));
});

assertRawJSONSyntaxError(undefined);
assertRawJSONSyntaxError({});
assertRawJSONSyntaxError([]);

assertRawJSONSyntaxError("");
assertRawJSONSyntaxError("\t123");
assertRawJSONSyntaxError("123\t");
assertRawJSONSyntaxError("\n123");
assertRawJSONSyntaxError("123\n");
assertRawJSONSyntaxError("\r123");
assertRawJSONSyntaxError("123\r");
assertRawJSONSyntaxError(" 123");
assertRawJSONSyntaxError("123 ");

assertRawJSONSyntaxError("{}");
assertRawJSONSyntaxError("[]");

