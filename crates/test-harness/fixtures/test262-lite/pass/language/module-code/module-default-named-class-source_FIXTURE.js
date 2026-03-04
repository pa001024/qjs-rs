export default class Counter {
  static base() {
    return 41;
  }
}

export const answer = Counter.base() + 1;
