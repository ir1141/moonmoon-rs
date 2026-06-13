import { describe, test, expect } from "bun:test";
import {
  safeLocalStorage,
  storageGet,
  storageRemove,
  storageSet,
} from "../static/lib/storage.js";

function fakeStorage() {
  const map = new Map();
  return {
    getItem: (key) => (map.has(key) ? map.get(key) : null),
    setItem: (key, value) => map.set(key, String(value)),
    removeItem: (key) => map.delete(key),
  };
}

// Simulates a browser where the storage object exists but every call throws
// (e.g. SecurityError with blocked site data, QuotaExceededError on write).
const throwingStorage = {
  getItem: () => {
    throw new Error("SecurityError");
  },
  setItem: () => {
    throw new Error("QuotaExceededError");
  },
  removeItem: () => {
    throw new Error("SecurityError");
  },
};

describe("storageGet", () => {
  test("returns the stored value", () => {
    const storage = fakeStorage();
    storage.setItem("k", "v");
    expect(storageGet(storage, "k")).toBe("v");
  });

  test("returns null for a missing key", () => {
    expect(storageGet(fakeStorage(), "missing")).toBe(null);
  });

  test("returns null when getItem throws", () => {
    expect(storageGet(throwingStorage, "k")).toBe(null);
  });

  test("returns null when storage is null", () => {
    expect(storageGet(null, "k")).toBe(null);
  });
});

describe("storageSet", () => {
  test("round-trips a value through storageGet", () => {
    const storage = fakeStorage();
    storageSet(storage, "k", "v");
    expect(storageGet(storage, "k")).toBe("v");
  });

  test("does not throw when setItem throws", () => {
    expect(() => storageSet(throwingStorage, "k", "v")).not.toThrow();
  });

  test("does not throw when storage is null", () => {
    expect(() => storageSet(null, "k", "v")).not.toThrow();
  });
});

describe("storageRemove", () => {
  test("removes a stored value", () => {
    const storage = fakeStorage();
    storageSet(storage, "k", "v");
    storageRemove(storage, "k");
    expect(storageGet(storage, "k")).toBe(null);
  });

  test("does not throw when removeItem throws", () => {
    expect(() => storageRemove(throwingStorage, "k")).not.toThrow();
  });

  test("does not throw when storage is null", () => {
    expect(() => storageRemove(null, "k")).not.toThrow();
  });
});

describe("safeLocalStorage", () => {
  test("does not throw outside a browser (no window)", () => {
    expect(() => safeLocalStorage()).not.toThrow();
    // bun's test runtime has no window, so the guarded lookup yields null.
    expect(safeLocalStorage()).toBe(null);
  });
});
