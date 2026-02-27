/*---
description: RegExp smoke covers constructor cloning, exec/test lastIndex, captures, and SyntaxError boundaries
---*/
var original = /foo/gi;
var clone = new RegExp(original);
assert.notSameValue(clone, original);
assert.sameValue(clone.toString(), '/foo/gi');

var overridden = new RegExp(original, 'ym');
assert.sameValue(overridden.toString(), '/foo/my');
assert.sameValue(overridden.multiline, true);
assert.sameValue(overridden.sticky, true);
assert.sameValue(overridden.global, false);

var allFlags = new RegExp('x', 'ygmius');
assert.sameValue(allFlags.toString(), '/x/gimsuy');

var global = /a/g;
global.lastIndex = 1;
var first = global.exec('ba');
assert.notSameValue(first, null);
assert.sameValue(first.index, 1);
assert.sameValue(global.lastIndex, 2);
assert.sameValue(global.exec('ba'), null);
assert.sameValue(global.lastIndex, 0);

var captures = /(a)(b)?/g.exec('ab a');
assert.sameValue(captures[0], 'ab');
assert.sameValue(captures[1], 'a');
assert.sameValue(captures[2], 'b');
assert.sameValue(captures.input, 'ab a');
assert.sameValue(captures.index, 0);

assert.throws(SyntaxError, function () {
  new RegExp('a', 'z');
});
assert.throws(SyntaxError, function () {
  new RegExp('a', 'gg');
});
assert.throws(SyntaxError, function () {
  new RegExp('(');
});
