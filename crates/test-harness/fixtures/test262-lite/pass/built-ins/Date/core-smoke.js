/*---
description: date baseline smoke
---*/

var ts = Date.UTC(2020, 0, 2, 3, 4, 5, 6);
var d = new Date(ts);

assert.sameValue(Date.length, 7);
assert.sameValue(Date.UTC.length, 7);
assert.sameValue(d.getTime(), ts);
assert.sameValue(d.toString(), 'Thu, 02 Jan 2020 03:04:05 GMT');
assert.sameValue(d.toUTCString(), 'Thu, 02 Jan 2020 03:04:05 GMT');
assert.sameValue(Date.parse('2020-01-02T03:04:05.006Z'), ts);
