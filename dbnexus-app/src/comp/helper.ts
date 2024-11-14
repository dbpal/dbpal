/// random throw of something
/// used in test

async function randomTestThrowFunction() {
  await delayedReturn(undefined, Math.random() * 900);
  const v = Math.random();
  if (v > 0.8) {
    throw new Error(`this is random error of ${v}`);
  }
}

async function voidTestFunction() {
  try {
    await randomTestThrowFunction();
  } catch (e) {
    throw e;
  }
}

function generateRandomString(length: number = 10): string {
  const characters =
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let result = '';
  for (let i = 0; i < length; i++) {
    const randomIndex = Math.floor(Math.random() * characters.length);
    result += characters[randomIndex];
  }

  return result;
}

function generateRandomUint8Array(length: number = 40): Uint8Array {
  let result = [];
  for (let i = 0; i < length; i++) {
    result.push(Math.floor(Math.random() * 255));
  }
  return new Uint8Array(result);
}

function delayedReturn(value: any, delay: number) {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve(value);
    }, delay);
  });
}

export {
  randomTestThrowFunction,
  generateRandomString,
  delayedReturn,
  voidTestFunction,
  generateRandomUint8Array,
};
