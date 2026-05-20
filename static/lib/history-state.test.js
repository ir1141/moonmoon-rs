import { describe, expect, test } from "bun:test";
import {
  buildHistoryEntries,
  readJsonStore,
  serializeHistoryRequest,
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

describe("buildHistoryEntries", () => {
  test("resume-only entries are in progress", () => {
    const entries = buildHistoryEntries(
      { v1: { time: 123.9, updated: 200 } },
      {},
    );

    expect(entries).toEqual([
      { id: "v1", state: "in_progress", time: 123, updated: 200 },
    ]);
  });

  test("watched-only entries are completed history", () => {
    const entries = buildHistoryEntries({}, { v2: { updated: 300 } });

    expect(entries).toEqual([{ id: "v2", state: "watched", updated: 300 }]);
  });

  test("newest mixed entry decides visible state", () => {
    const entries = buildHistoryEntries(
      {
        resume_newer: { time: 44, updated: 400 },
        watched_newer: { time: 88, updated: 200 },
      },
      {
        resume_newer: { updated: 100 },
        watched_newer: { updated: 500 },
      },
    );

    expect(entries).toEqual([
      { id: "watched_newer", state: "watched", updated: 500 },
      { id: "resume_newer", state: "in_progress", time: 44, updated: 400 },
    ]);
  });

  test("history request serializes ids times and states in the same order", () => {
    const entries = [
      { id: "recent", state: "watched", updated: 500 },
      { id: "resume", state: "in_progress", time: 42, updated: 400 },
    ];

    expect(serializeHistoryRequest(entries, "recent").toString()).toBe(
      "ids=recent%2Cresume&times=%2C42&states=watched%2Cin_progress&sort=recent",
    );

    expect(serializeHistoryRequest(entries, "game").get("sort")).toBe("game");
  });
});
