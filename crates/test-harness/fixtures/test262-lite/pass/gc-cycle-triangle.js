/*---
description: gc stress keeps triangle cycle reachable through retained closure reads
---*/
let a = { name: "a" };
let b = { name: "b" };
let c = { name: "c" };
a.next = b;
b.next = c;
c.next = a;

function readCycle(rounds) {
  let cursor = a;
  let count = 0;
  for (let i = 0; i < rounds; i = i + 1) {
    if (cursor.name === "a" || cursor.name === "b" || cursor.name === "c") {
      count = count + 1;
    }
    cursor = cursor.next;
  }
  return count;
}

let total = 0;
for (let pass = 0; pass < 12; pass = pass + 1) {
  let temp = { id: pass, hold: { v: pass + 1 } };
  total = total + temp.hold.v;
}

readCycle(21) + total;