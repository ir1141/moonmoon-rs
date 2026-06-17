import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { fetchChannelEmotes, lookupEmote } from "./emote-client.js";

let originalFetch;
let lastUrl;

beforeEach(() => {
  originalFetch = globalThis.fetch;
  lastUrl = null;
});

afterEach(() => {
  globalThis.fetch = originalFetch;
});

function mockJson(body, status = 200) {
  globalThis.fetch = /** @type {any} */ (
    (url) => {
      lastUrl = url;
      return Promise.resolve({
        ok: status >= 200 && status < 300,
        status,
        json: () => Promise.resolve(body),
      });
    }
  );
}

describe("fetchChannelEmotes", () => {
  test("returns the emotes map from /api/emotes/channel", async () => {
    mockJson({
      emotes: {
        PogU: { url: "https://x/1", provider: "7TV" },
        catJAM: { url: "https://x/2", provider: "BTTV", owner: "X" },
      },
    });
    const out = await fetchChannelEmotes();
    expect(lastUrl).toBe("/api/emotes/channel");
    expect(out.PogU.provider).toBe("7TV");
    expect(out.catJAM.owner).toBe("X");
  });

  test("returns empty object on non-OK response", async () => {
    mockJson({}, 500);
    expect(await fetchChannelEmotes()).toEqual({});
  });

  test("returns empty object on fetch throw", async () => {
    globalThis.fetch = /** @type {any} */ (
      () => Promise.reject(new Error("net"))
    );
    expect(await fetchChannelEmotes()).toEqual({});
  });
});

describe("lookupEmote", () => {
  test("returns hit record from /api/emotes/lookup/{name}", async () => {
    mockJson({ hit: true, url: "https://x/3", provider: "FFZ", owner: "Z" });
    const out = await lookupEmote("ZreknarF");
    expect(lastUrl).toBe("/api/emotes/lookup/ZreknarF");
    expect(out).toEqual({
      hit: true,
      url: "https://x/3",
      provider: "FFZ",
      owner: "Z",
    });
  });

  test("returns miss record verbatim", async () => {
    mockJson({ hit: false });
    expect(await lookupEmote("notreal")).toEqual({ hit: false });
  });

  test("percent-encodes names with special chars", async () => {
    mockJson({ hit: false });
    await lookupEmote(":tf:");
    expect(lastUrl).toBe("/api/emotes/lookup/%3Atf%3A");
  });

  test("returns transient miss on fetch throw", async () => {
    globalThis.fetch = /** @type {any} */ (
      () => Promise.reject(new Error("net"))
    );
    const out = await lookupEmote("x");
    expect(out.hit).toBe(false);
    expect(out.transient).toBe(true);
  });
});
