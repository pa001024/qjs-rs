export default function* Gen() {
  yield 40;
  yield 2;
}

const iter = Gen();
export const total = iter.next().value + iter.next().value;
