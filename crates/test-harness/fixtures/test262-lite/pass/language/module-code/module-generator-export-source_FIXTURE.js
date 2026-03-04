export function* values() {
  yield 40;
  yield 2;
}

const iter = values();
export const total = iter.next().value + iter.next().value;
