import { afterAll, beforeAll, describe, expect, test } from "bun:test";

// Regression test for 7bcf557: player.js and sync.js defined local
// storageGet/storageSet wrappers that recursed into THEMSELVES instead of
// touching localStorage. Every read/write became a silent no-op (the
// RangeError was swallowed), so resume positions, watched state, prefs, and
// the sync token were never saved — and no test failed, because the harness
// only covered the pure helpers in static/lib/, never the glue.
//
// This file closes that gap for sync.js by importing the real module against
// a fake window.localStorage and asserting writes actually land in it.
// player.js needs a full DOM to import, so it gets a source-level tripwire
// below instead.

const backing = new Map();
const fakeLocalStorage = {
  getItem: (key) => (backing.has(key) ? backing.get(key) : null),
  setItem: (key, value) => backing.set(key, String(value)),
  removeItem: (key) => backing.delete(key),
};

const TOKEN_KEY = "moonmoon_sync_token";
// isValidToken requires /^[A-Z2-7]{26,32}$/.
const TOKEN = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

let sync;

beforeAll(async () => {
  globalThis.window = /** @type {any} */ ({
    localStorage: fakeLocalStorage,
    addEventListener: () => {},
    dispatchEvent: () => true,
  });
  globalThis.document = /** @type {any} */ ({ getElementById: () => null });

  // sync.js starts a 2s storage poll at module eval; keep timers out of the
  // test process.
  const realSetInterval = globalThis.setInterval;
  globalThis.setInterval = /** @type {any} */ (() => 0);
  try {
    await import("../static/sync.js");
  } finally {
    globalThis.setInterval = realSetInterval;
  }
  sync = globalThis.window.__moonmoonSync;
});

afterAll(() => {
  // Other test files assert browserless behavior (safeLocalStorage() === null
  // without a window); don't leak the stubs into them.
  delete globalThis.window;
  delete globalThis.document;
});

describe("sync.js localStorage round-trip", () => {
  test("setToken writes the token into localStorage", () => {
    sync.setToken(TOKEN);
    expect(backing.get(TOKEN_KEY)).toBe(TOKEN);
  });

  test("getToken reads the persisted token back", () => {
    expect(sync.getToken()).toBe(TOKEN);
  });

  test("setToken with an invalid value removes the stored token", () => {
    sync.setToken("");
    expect(backing.has(TOKEN_KEY)).toBe(false);
    expect(sync.getToken()).toBe("");
  });
});

describe("storage wrapper shadowing tripwire", () => {
  // The 7bcf557 bug shape: a module-local `function storageGet(...)`
  // shadowing the lib import and recursing. If anyone reintroduces local
  // wrappers in the storage-touching glue, fail loudly.
  for (const file of ["player.js", "sync.js"]) {
    test(`static/${file} routes storage access through lib/storage.js`, async () => {
      const src = await Bun.file(
        new URL(`../static/${file}`, import.meta.url),
      ).text();
      expect(src).not.toMatch(/function\s+storage(Get|Set|Remove)\s*\(/);
      expect(src).toMatch(/from\s+"\.\/lib\/storage\.js"/);
    });
  }
});
