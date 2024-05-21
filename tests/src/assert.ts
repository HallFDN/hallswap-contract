export function assertEquals<T>(actual: T, expected: T, errorMsg?: string) {
  if (actual !== expected) {
    throw new Error(
      "Test failed! " +
        (errorMsg ?? `Expected [${expected}] but got [${actual}]`)
    );
  }
}

export async function assertThrows(fn: () => any, errorMsg?: string) {
  try {
    await fn();
  } catch (err) {
    return;
  }
  throw new Error(
    "Test failed! " + (errorMsg ?? `Expected error to be thrown`)
  );
}
