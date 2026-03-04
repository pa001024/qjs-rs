/*---
description: JSON.stringify can embed BigInt via JSON.rawJSON replacer
---*/
if (typeof BigInt === "function") {
  var tooBig = BigInt(Number.MAX_SAFE_INTEGER) + 2n;
  var result = JSON.stringify({ tooBig: tooBig }, function (key, value) {
    return typeof value === "bigint" ? JSON.rawJSON(value) : value;
  });
  assert.sameValue(result, '{"tooBig":9007199254740993}');
}

