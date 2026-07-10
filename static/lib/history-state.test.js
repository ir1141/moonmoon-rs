import { describe, expect, test } from "bun:test";
import wireFixture from "../../tests/fixtures/history-request.json";
import {
  HISTORY_KEY,
  LEGACY_RESUME_KEY,
  LEGACY_WATCHED_KEY,
  MAX_HISTORY_ENTRIES,
  buildHistoryEntries,
  buildHistoryRequest,
  historyFromBlob,
  loadHistoryStore,
  markWatched,
  mergeHistory,
  migrateLegacyStores,
  normalizeHistoryEntry,
  readJsonStore,
  resumePercent,
  saveResumePosition,
} from "./history-state.js";

describe("readJsonStore", () => {
  test("reads object stores and falls back for invalid values", () => {
    const storage = new Map([
      ["valid", JSON.stringify({ v1: { updated: 100 } })],
      ["array", JSON.stringify([])],
      ["broken", "{"],
    ]);

    expect(readJsonStore(storage, "valid")).toEqual({ v1: { updated: 100 } });
    expect(readJsonStore(storage, "array")).toEqual({});
    expect(readJsonStore(storage, "broken")).toEqual({});
    expect(readJsonStore(storage, "missing")).toEqual({});
  });
});

describe("normalizeHistoryEntry", () => {
  test("keeps valid in-progress entries and floors time", () => {
    expect(
      normalizeHistoryEntry({
        state: "in_progress",
        time: 123.9,
        updated: 200,
        part: 2,
        localTime: 45.5,
      }),
    ).toEqual({
      state: "in_progress",
      time: 123,
      updated: 200,
      part: 2,
      localTime: 45.5,
    });
  });

  test("drops junk positions but keeps the entry", () => {
    expect(
      normalizeHistoryEntry({ state: "in_progress", time: "x", updated: 1 }),
    ).toEqual({ state: "in_progress", time: 0, updated: 1 });
  });

  test("watched entries carry no position", () => {
    expect(
      normalizeHistoryEntry({ state: "watched", time: 55, updated: 300 }),
    ).toEqual({ state: "watched", updated: 300 });
  });

  test("rejects unknown states and missing timestamps", () => {
    expect(normalizeHistoryEntry({ state: "liked", updated: 1 })).toBeNull();
    expect(normalizeHistoryEntry({ state: "watched" })).toBeNull();
    expect(normalizeHistoryEntry({ state: "watched", updated: -1 })).toBeNull();
    expect(normalizeHistoryEntry(null)).toBeNull();
    expect(normalizeHistoryEntry(true)).toBeNull();
  });
});

describe("migrateLegacyStores", () => {
  test("converts both legacy stores including pre-object watched formats", () => {
    const migrated = migrateLegacyStores(
      { v1: { time: 123.9, updated: 200, part: 1, localTime: 7.5 } },
      { v2: true, v3: 400, v4: { updated: 500 } },
    );

    expect(migrated).toEqual({
      v1: { state: "in_progress", time: 123, updated: 200, part: 1, localTime: 7.5 },
      v2: { state: "watched", updated: 0 },
      v3: { state: "watched", updated: 400 },
      v4: { state: "watched", updated: 500 },
    });
  });

  test("newer side wins per id; in-progress wins ties", () => {
    const migrated = migrateLegacyStores(
      {
        resume_newer: { time: 44, updated: 400 },
        watched_newer: { time: 88, updated: 200 },
        tied: { time: 10, updated: 300 },
      },
      {
        resume_newer: { updated: 100 },
        watched_newer: { updated: 500 },
        tied: { updated: 300 },
      },
    );

    expect(migrated.resume_newer.state).toBe("in_progress");
    expect(migrated.watched_newer).toEqual({ state: "watched", updated: 500 });
    expect(migrated.tied.state).toBe("in_progress");
  });

  test("drops junk on both sides", () => {
    expect(migrateLegacyStores({ v1: null, v2: 7 }, { v3: false })).toEqual(
      {},
    );
  });
});

describe("loadHistoryStore", () => {
  test("prefers the unified key when present", () => {
    const storage = new Map([
      [HISTORY_KEY, JSON.stringify({ v1: { state: "watched", updated: 9 } })],
      [LEGACY_RESUME_KEY, JSON.stringify({ v2: { time: 5, updated: 1 } })],
    ]);

    expect(loadHistoryStore(storage)).toEqual({
      v1: { state: "watched", updated: 9 },
    });
    expect(storage.has(LEGACY_RESUME_KEY)).toBe(true);
  });

  test("migrates legacy keys once and deletes them", () => {
    const storage = new Map([
      [LEGACY_RESUME_KEY, JSON.stringify({ v1: { time: 60, updated: 100 } })],
      [LEGACY_WATCHED_KEY, JSON.stringify({ v2: { updated: 200 } })],
    ]);

    const store = loadHistoryStore(storage);

    expect(store).toEqual({
      v1: { state: "in_progress", time: 60, updated: 100 },
      v2: { state: "watched", updated: 200 },
    });
    expect(JSON.parse(storage.get(HISTORY_KEY))).toEqual(store);
    expect(storage.has(LEGACY_RESUME_KEY)).toBe(false);
    expect(storage.has(LEGACY_WATCHED_KEY)).toBe(false);
  });

  test("no keys at all reads empty without writing", () => {
    const storage = new Map();
    expect(loadHistoryStore(storage)).toEqual({});
    expect(storage.size).toBe(0);
  });

  test("corrupt unified store degrades to empty", () => {
    const storage = new Map([[HISTORY_KEY, "{"]]);
    expect(loadHistoryStore(storage)).toEqual({});
  });
});

describe("saveResumePosition / markWatched", () => {
  test("saving a position replaces a watched entry", () => {
    const store = { v1: { state: "watched", updated: 100 } };
    const next = saveResumePosition(
      store,
      "v1",
      { time: 90.7, part: 0, localTime: 90.7 },
      200,
    );

    expect(next.v1).toEqual({
      state: "in_progress",
      time: 90,
      updated: 200,
      part: 0,
      localTime: 90.7,
    });
  });

  test("startup noise cannot replace meaningful progress", () => {
    const meaningful = {
      v1: { state: "in_progress", time: 937, updated: 10_000 },
    };
    expect(
      saveResumePosition(
        meaningful,
        "v1",
        { time: 0, part: 0, localTime: 0 },
        20_000,
      ),
    ).toEqual(meaningful);
    expect(
      saveResumePosition({}, "v1", { time: 5, part: 0, localTime: 5 }, 20_000),
    ).toEqual({});
  });

  test("marking watched drops the position", () => {
    const store = {
      v1: { state: "in_progress", time: 90, updated: 100, part: 1 },
    };
    expect(markWatched(store, "v1", 200)).toEqual({
      v1: { state: "watched", updated: 200 },
    });
  });

  test("evicts the oldest entries past the cap", () => {
    const store = {};
    for (let i = 0; i < MAX_HISTORY_ENTRIES; i++) {
      store[`v${i}`] = { state: "watched", updated: i + 1000 };
    }

    const next = markWatched(store, "newest", 999999);

    expect(Object.keys(next).length).toBe(MAX_HISTORY_ENTRIES);
    expect(next.newest).toBeDefined();
    expect(next.v0).toBeUndefined();
    expect(next.v1).toBeDefined();
  });
});

describe("mergeHistory", () => {
  test("newer remote entries win and flag a change", () => {
    const { merged, changed } = mergeHistory(
      { v1: { state: "in_progress", time: 10, updated: 100 } },
      {
        v1: { state: "watched", updated: 200 },
        v2: { state: "in_progress", time: 5, updated: 50 },
      },
    );

    expect(changed).toBe(true);
    expect(merged.v1).toEqual({ state: "watched", updated: 200 });
    expect(merged.v2.state).toBe("in_progress");
  });

  test("local wins ties and stale remote entries change nothing", () => {
    const local = {
      v1: { state: "in_progress", time: 10, updated: 100 },
      v2: { state: "watched", updated: 300 },
    };
    const { merged, changed } = mergeHistory(local, {
      v1: { state: "watched", updated: 100 },
      v2: { state: "in_progress", time: 1, updated: 299 },
    });

    expect(changed).toBe(false);
    expect(merged).toEqual(local);
  });

  test("meaningful progress beats newer startup noise", () => {
    const meaningful = { state: "in_progress", time: 937, updated: 10_000 };
    const noise = { state: "in_progress", time: 0, updated: 20_000 };

    expect(mergeHistory({ v1: noise }, { v1: meaningful })).toEqual({
      merged: { v1: meaningful },
      changed: true,
    });
    expect(mergeHistory({ v1: meaningful }, { v1: noise })).toEqual({
      merged: { v1: meaningful },
      changed: false,
    });
  });

  test("junk remote input changes nothing", () => {
    const local = { v1: { state: "watched", updated: 1 } };
    expect(mergeHistory(local, null).changed).toBe(false);
    expect(mergeHistory(local, { v2: "junk" }).changed).toBe(false);
  });
});

describe("historyFromBlob", () => {
  test("reads the v2 shape", () => {
    expect(
      historyFromBlob({
        v: 2,
        history: { v1: { state: "watched", updated: 5 } },
      }),
    ).toEqual({ v1: { state: "watched", updated: 5 } });
  });

  test("reads the legacy split-store shape", () => {
    expect(
      historyFromBlob({
        resume: { v1: { time: 30, updated: 100 } },
        watched: { v2: true },
      }),
    ).toEqual({
      v1: { state: "in_progress", time: 30, updated: 100 },
      v2: { state: "watched", updated: 0 },
    });
  });

  test("junk blobs read as empty", () => {
    expect(historyFromBlob(null)).toEqual({});
    expect(historyFromBlob([])).toEqual({});
    expect(historyFromBlob({ v: 2, history: [] })).toEqual({});
  });
});

describe("resumePercent", () => {
  test("clamps to 0..100 and rejects junk durations", () => {
    expect(resumePercent(25, 100)).toBe(25);
    expect(resumePercent(150, 100)).toBe(100);
    expect(resumePercent(-5, 100)).toBe(0);
    expect(resumePercent(50, 0)).toBe(0);
    expect(resumePercent("x", 100)).toBe(0);
  });
});

describe("buildHistoryEntries", () => {
  test("flattens the store most recently updated first", () => {
    const entries = buildHistoryEntries({
      older: { state: "in_progress", time: 44, updated: 400, part: 2 },
      newer: { state: "watched", updated: 500 },
    });

    expect(entries).toEqual([
      { id: "newer", state: "watched", updated: 500 },
      { id: "older", state: "in_progress", time: 44, updated: 400 },
    ]);
  });

  test("ties order by id for a stable request", () => {
    const entries = buildHistoryEntries({
      b: { state: "watched", updated: 100 },
      a: { state: "watched", updated: 100 },
    });

    expect(entries.map((entry) => entry.id)).toEqual(["a", "b"]);
  });

  test("the request body matches the cross-language wire fixture", () => {
    const entries = [
      { id: "recent", state: "watched", updated: 500 },
      { id: "resume", state: "in_progress", time: 42.9, updated: 400 },
    ];

    expect(buildHistoryRequest(entries, "recent")).toEqual(wireFixture);
    expect(buildHistoryRequest(entries, "game").sort).toBe("game");
    expect(buildHistoryRequest(entries, "junk").sort).toBe("recent");
  });
});
