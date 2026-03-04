export default async function* Gen() {
  yield 1;
}

export const genType = typeof Gen;
