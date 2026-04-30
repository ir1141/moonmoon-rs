import { describe, test, expect } from "bun:test";
import { mergeResume } from "../static/lib/resume.js";

describe("mergeResume", () => {
  test.each([null, undefined, "oops", 42])(
    "returns local unchanged when remote is %p",
    (remote) => {
      const local = { a: { updated: 1 } };
      const result = mergeResume(local, remote);
      expect(result.merged).toEqual(local);
      expect(result.changed).toBe(false);
    },
  );

  test("adds remote-only keys and reports changed", () => {
    const result = mergeResume({}, { a: { updated: 1 } });
    expect(result.merged).toEqual({ a: { updated: 1 } });
    expect(result.changed).toBe(true);
  });

  test("takes the entry with the larger updated timestamp (and reports changed)", () => {
    const local = { a: { updated: 10, time: "old" } };
    const remote = { a: { updated: 20, time: "new" } };
    const result = mergeResume(local, remote);
    expect(result.merged.a).toEqual({ updated: 20, time: "new" });
    expect(result.changed).toBe(true);
  });

  test("keeps local when local is newer (no change reported)", () => {
    const local = { a: { updated: 30, time: "local" } };
    const remote = { a: { updated: 10, time: "remote" } };
    const result = mergeResume(local, remote);
    expect(result.merged.a).toEqual({ updated: 30, time: "local" });
    expect(result.changed).toBe(false);
  });

  test("does not mutate local or remote", () => {
    const local = { a: { updated: 1 } };
    const remote = { a: { updated: 2 } };
    const localSnap = JSON.parse(JSON.stringify(local));
    const remoteSnap = JSON.parse(JSON.stringify(remote));
    mergeResume(local, remote);
    expect(local).toEqual(localSnap);
    expect(remote).toEqual(remoteSnap);
  });
});
