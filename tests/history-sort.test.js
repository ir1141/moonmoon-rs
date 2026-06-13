import { describe, expect, test } from "bun:test";
import {
  defaultHistorySort,
  normalizeHistorySort,
  readHistorySort,
  writeHistorySort,
} from "../static/lib/history-sort.js";

describe("normalizeHistorySort", () => {
  test("keeps supported history sort values", () => {
    expect(normalizeHistorySort("recent")).toBe("recent");
    expect(normalizeHistorySort("game")).toBe("game");
  });

  test("falls back to the default for unsupported values", () => {
    expect(normalizeHistorySort(null)).toBe(defaultHistorySort);
    expect(normalizeHistorySort("oldest")).toBe(defaultHistorySort);
    expect(normalizeHistorySort("")).toBe(defaultHistorySort);
  });
});

describe("history sort storage", () => {
  test("reads a valid stored sort", () => {
    const storage = new Map([["moonmoon_history_sort", "game"]]);

    expect(readHistorySort(storage)).toBe("game");
  });

  test("writes normalized sort values", () => {
    const storage = new Map();

    expect(writeHistorySort(storage, "oldest")).toBe(defaultHistorySort);
    expect(storage.get("moonmoon_history_sort")).toBe(defaultHistorySort);
  });

  test("ignores unavailable storage", () => {
    const throwingStorage = {
      get() {
        throw new Error("no storage");
      },
      set() {
        throw new Error("no storage");
      },
    };

    expect(readHistorySort(throwingStorage)).toBe(defaultHistorySort);
    expect(writeHistorySort(throwingStorage, "game")).toBe("game");
  });

  // The localStorage *global* itself can throw (SecurityError in
  // storage-blocking browsers) or be absent (bun) — falling back to it must
  // go through a guard, not a bare reference.
  test("defaults do not touch a bare localStorage global", () => {
    expect(() => readHistorySort()).not.toThrow();
    expect(readHistorySort()).toBe(defaultHistorySort);
    expect(() => writeHistorySort(null, "game")).not.toThrow();
    expect(writeHistorySort(null, "game")).toBe("game");
  });
});
