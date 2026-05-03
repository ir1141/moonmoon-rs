// Per-provider request builders + response parsers for the lazy emote lookup.
// Kept pure (no fetch, no DOM) so they can be unit-tested with fixture JSON.
//
// buildSearchUrl(provider, name) → { url, method, body?, headers? }
// parseSearchResponse(provider, json, name) → { hit, url?, provider?, owner? }
//
// Each parser MUST require an exact, case-sensitive name match — provider
// search endpoints return prefix/substring matches and we only render an
// emote when we're sure the user typed the right token.

const SEVENTV_SEARCH_QUERY =
  "query SearchEmotes($query: String!, $page: Int, $sort: Sort, $limit: Int, $filter: EmoteSearchFilter) {\n" +
  "  emotes(query: $query, page: $page, sort: $sort, limit: $limit, filter: $filter) {\n" +
  "    count\n" +
  "    items {\n" +
  "      id name\n" +
  "      host { url }\n" +
  "      owner { display_name connections { platform display_name } }\n" +
  "    }\n" +
  "  }\n" +
  "}";

export function buildSearchUrl(provider, name) {
  if (provider === "7TV") {
    return {
      url: "https://7tv.io/v3/gql",
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        operationName: "SearchEmotes",
        variables: {
          query: name,
          limit: 4,
          page: 1,
          sort: { value: "popularity", order: "DESCENDING" },
          filter: {
            category: "TOP",
            exact_match: true,
            case_sensitive: true,
            ignore_tags: false,
            zero_width: false,
            animated: false,
            aspect_ratio: "",
          },
        },
        query: SEVENTV_SEARCH_QUERY,
      }),
    };
  }
  if (provider === "BTTV") {
    return {
      url:
        "https://api.betterttv.net/3/emotes/shared/search?query=" +
        encodeURIComponent(name) +
        "&offset=0&limit=10",
      method: "GET",
    };
  }
  if (provider === "FFZ") {
    return {
      url:
        "https://api.frankerfacez.com/v1/emotes?q=" +
        encodeURIComponent(name) +
        "&sensitive=false&sort=count-desc&page=1",
      method: "GET",
    };
  }
  throw new Error("Unknown emote provider: " + provider);
}

function pick7TVOwner(owner) {
  if (!owner) return null;
  const conns = owner.connections || [];
  for (const c of conns) {
    if (c && c.platform === "TWITCH" && c.display_name) return c.display_name;
  }
  return owner.display_name || null;
}

export function parseSearchResponse(provider, json, name) {
  if (provider === "7TV") {
    const items =
      (json && json.data && json.data.emotes && json.data.emotes.items) || [];
    for (const it of items) {
      if (it && it.name === name && it.host && it.host.url) {
        return {
          hit: true,
          url: "https:" + it.host.url + "/1x.webp",
          provider: "7TV",
          owner: pick7TVOwner(it.owner),
        };
      }
    }
    return { hit: false };
  }
  if (provider === "BTTV") {
    const items = Array.isArray(json) ? json : [];
    for (const it of items) {
      if (it && it.code === name && it.id) {
        return {
          hit: true,
          url: "https://cdn.betterttv.net/emote/" + it.id + "/1x",
          provider: "BTTV",
          owner: (it.user && it.user.displayName) || null,
        };
      }
    }
    return { hit: false };
  }
  if (provider === "FFZ") {
    const items = (json && json.emoticons) || [];
    for (const it of items) {
      if (it && it.name === name && it.urls) {
        const url = it.urls["1"] || it.urls["2"] || it.urls["4"];
        if (!url) continue;
        return {
          hit: true,
          url: "https:" + url,
          provider: "FFZ",
          owner: (it.owner && it.owner.display_name) || null,
        };
      }
    }
    return { hit: false };
  }
  throw new Error("Unknown emote provider: " + provider);
}

export const PROVIDERS = ["7TV", "BTTV", "FFZ"];
