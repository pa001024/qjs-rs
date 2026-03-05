/*---
description: JSON.rawJSON and JSON.isRawJSON basic behavior
---*/
assert.sameValue(JSON.stringify(JSON.rawJSON(1)), "1");
assert.sameValue(JSON.stringify(JSON.rawJSON(1.1)), "1.1");
assert.sameValue(JSON.stringify(JSON.rawJSON(null)), "null");
assert.sameValue(JSON.stringify(JSON.rawJSON(true)), "true");
assert.sameValue(JSON.stringify(JSON.rawJSON(false)), "false");
assert.sameValue(JSON.stringify(JSON.rawJSON('"foo"')), '"foo"');

assert.sameValue(JSON.stringify({ a: JSON.rawJSON(37) }), '{"a":37}');
assert.sameValue(
  JSON.stringify({ x: JSON.rawJSON(1), y: JSON.rawJSON(2) }),
  '{"x":1,"y":2}'
);
assert.sameValue(
  JSON.stringify([JSON.rawJSON('"1"'), JSON.rawJSON(true), JSON.rawJSON(null)]),
  '["1",true,null]'
);

assert.sameValue(JSON.isRawJSON(JSON.rawJSON(1)), true);
assert.sameValue(JSON.isRawJSON(JSON.rawJSON('"123"')), true);
assert.sameValue(JSON.isRawJSON(1), false);
assert.sameValue(JSON.isRawJSON(undefined), false);
assert.sameValue(JSON.isRawJSON(null), false);
assert.sameValue(JSON.isRawJSON({ rawJSON: "1" }), false);

