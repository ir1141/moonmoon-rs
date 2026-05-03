import { describe, test, expect } from "bun:test";
import {
  buildSearchUrl,
  parseSearchResponse,
} from "./emote-providers.js";

describe("buildSearchUrl", () => {
  test("7TV returns gql endpoint and POST body", () => {
    const req = buildSearchUrl("7TV", "TANIMURA");
    expect(req.url).toBe("https://7tv.io/v3/gql");
    expect(req.method).toBe("POST");
    const body = JSON.parse(req.body);
    expect(body.variables.query).toBe("TANIMURA");
    expect(body.operationName).toBe("SearchEmotes");
  });

  test("BTTV returns shared-search GET URL with query param", () => {
    const req = buildSearchUrl("BTTV", "catJAM");
    expect(req.method).toBe("GET");
    expect(req.url).toBe(
      "https://api.betterttv.net/3/emotes/shared/search?query=catJAM&offset=0&limit=10",
    );
  });

  test("BTTV percent-encodes query", () => {
    const req = buildSearchUrl("BTTV", "foo bar");
    expect(req.url).toContain("query=foo%20bar");
  });

  test("FFZ returns search GET URL", () => {
    const req = buildSearchUrl("FFZ", "ZreknarF");
    expect(req.method).toBe("GET");
    expect(req.url).toBe(
      "https://api.frankerfacez.com/v1/emotes?q=ZreknarF&sensitive=false&sort=count-desc&page=1",
    );
  });

  test("unknown provider throws", () => {
    expect(() => buildSearchUrl("YouTube", "x")).toThrow();
  });
});

describe("parseSearchResponse 7TV", () => {
  test("returns hit on exact case-sensitive match", () => {
    const json = {
      data: {
        emotes: {
          items: [
            {
              id: "1",
              name: "TANIMURA",
              host: { url: "//cdn.7tv.app/emote/abc" },
              owner: {
                display_name: "tanimuraXYZ",
                connections: [
                  { platform: "TWITCH", display_name: "TanimuraTV" },
                ],
              },
            },
          ],
        },
      },
    };
    expect(parseSearchResponse("7TV", json, "TANIMURA")).toEqual({
      hit: true,
      url: "https://cdn.7tv.app/emote/abc/1x.webp",
      provider: "7TV",
      owner: "TanimuraTV",
    });
  });

  test("returns miss when no exact match", () => {
    const json = {
      data: { emotes: { items: [{ name: "tanimura", host: { url: "//x" } }] } },
    };
    expect(parseSearchResponse("7TV", json, "TANIMURA")).toEqual({ hit: false });
  });

  test("returns miss on empty items", () => {
    expect(parseSearchResponse("7TV", { data: { emotes: { items: [] } } }, "x"))
      .toEqual({ hit: false });
  });
});

describe("parseSearchResponse BTTV", () => {
  test("returns hit on exact code match", () => {
    const json = [
      {
        id: "5f1b0186cf6d2144653d2970",
        code: "catJAM",
        user: { displayName: "OmegaPepega" },
      },
      { id: "other", code: "catjam", user: {} },
    ];
    expect(parseSearchResponse("BTTV", json, "catJAM")).toEqual({
      hit: true,
      url: "https://cdn.betterttv.net/emote/5f1b0186cf6d2144653d2970/1x",
      provider: "BTTV",
      owner: "OmegaPepega",
    });
  });

  test("returns miss when no exact match", () => {
    const json = [{ id: "1", code: "catjam", user: {} }];
    expect(parseSearchResponse("BTTV", json, "catJAM")).toEqual({ hit: false });
  });

  test("returns miss on empty array", () => {
    expect(parseSearchResponse("BTTV", [], "x")).toEqual({ hit: false });
  });

  test("hit without owner sets owner to null", () => {
    const json = [{ id: "1", code: "catJAM" }];
    expect(parseSearchResponse("BTTV", json, "catJAM")).toEqual({
      hit: true,
      url: "https://cdn.betterttv.net/emote/1/1x",
      provider: "BTTV",
      owner: null,
    });
  });
});

describe("parseSearchResponse FFZ", () => {
  test("returns hit on exact name match", () => {
    const json = {
      emoticons: [
        {
          id: 28138,
          name: "ZreknarF",
          urls: { 1: "//cdn.frankerfacez.com/emote/28138/1" },
          owner: { display_name: "Zreknarf" },
        },
      ],
    };
    expect(parseSearchResponse("FFZ", json, "ZreknarF")).toEqual({
      hit: true,
      url: "https://cdn.frankerfacez.com/emote/28138/1",
      provider: "FFZ",
      owner: "Zreknarf",
    });
  });

  test("falls back through url sizes 1 → 2 → 4", () => {
    const json = {
      emoticons: [
        { id: 1, name: "X", urls: { 4: "//u/4" }, owner: { display_name: "o" } },
      ],
    };
    expect(parseSearchResponse("FFZ", json, "X").url).toBe("https://u/4");
  });

  test("returns miss on no exact match", () => {
    const json = { emoticons: [{ id: 1, name: "zreknarf", urls: { 1: "//x" } }] };
    expect(parseSearchResponse("FFZ", json, "ZreknarF")).toEqual({ hit: false });
  });

  test("returns miss when emoticons missing", () => {
    expect(parseSearchResponse("FFZ", {}, "x")).toEqual({ hit: false });
  });
});
