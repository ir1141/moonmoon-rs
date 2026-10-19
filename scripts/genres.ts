// Regenerates data/genres.json: every chapter name in the archive mapped to
// short genre labels via Twitch (box-art id → igdb_id) and IGDB.
//
//   bun --env-file=../vodka/.env scripts/genres.ts [--refresh]
//
// Existing entries are kept (hand edits win) unless --refresh is passed.

const OUT = new URL("../data/genres.json", import.meta.url);
const ARCHIVE = "https://archive.overpowered.tv/api/v1/moonmoon/vods";
const PAGE_SIZE = 50;

const GENRE_LABELS: Record<string, string> = {
  "Role-playing (RPG)": "RPG",
  Strategy: "Strategy",
  "Real Time Strategy (RTS)": "Strategy",
  "Turn-based strategy (TBS)": "Strategy",
  Tactical: "Strategy",
  "Hack and slash/Beat 'em up": "Action",
  Shooter: "Shooter",
  Platform: "Platformer",
  Puzzle: "Puzzle",
  Adventure: "Adventure",
  "Point-and-click": "Adventure",
  Simulator: "Simulation",
  Racing: "Racing",
  Sport: "Sports",
  Fighting: "Fighting",
  "Card & Board Game": "Card & Board",
  "Visual Novel": "Visual Novel",
  MOBA: "MOBA",
  Music: "Rhythm",
  Arcade: "Arcade",
  Pinball: "Arcade",
  "Quiz/Trivia": "Party",
};
const THEME_LABELS: Record<string, string> = {
  Horror: "Horror",
  Survival: "Survival",
};
const LABEL_ORDER = [
  ...new Set([
    ...Object.values(GENRE_LABELS),
    ...Object.values(THEME_LABELS),
    "Non-gaming",
  ]),
];

const NON_GAMING = new Set([
  "just chatting",
  "special events",
  "art",
  "music",
  "irl",
  "talk shows & podcasts",
  "travel & outdoors",
  "food & drink",
  "asmr",
  "sports",
  "science & technology",
  "makers & crafting",
  "watch parties",
  "pools, hot tubs, and beaches",
]);

type Mapping = Record<string, string[]>;

function env(...names: string[]): string {
  for (const n of names) if (process.env[n]) return process.env[n]!;
  throw new Error(`missing env: ${names.join(" or ")}`);
}

async function json<T>(url: string, init?: RequestInit): Promise<T> {
  for (let attempt = 0; ; attempt++) {
    const res = await fetch(url, init);
    if (res.status === 429 && attempt < 5) {
      await Bun.sleep(1000 * 2 ** attempt);
      continue;
    }
    if (!res.ok) throw new Error(`${res.status} ${url}: ${await res.text()}`);
    return res.json() as Promise<T>;
  }
}

async function loadChapterGames(): Promise<Map<string, string | null>> {
  const games = new Map<string, string | null>();
  for (let page = 1, pages = 1; page <= pages; page++) {
    const resp = await json<{ data: any[]; meta: { total: number } }>(
      `${ARCHIVE}?page=${page}&limit=${PAGE_SIZE}&sort=created_at&order=desc`,
    );
    pages = Math.ceil(resp.meta.total / PAGE_SIZE);
    for (const vod of resp.data) {
      for (const ch of vod.chapters ?? []) {
        if (!ch.name) continue;
        const key = ch.name.toLowerCase();
        const twitchId = /ttv-boxart\/(\d+)[_-]/.exec(ch.image ?? "")?.[1];
        if (!games.get(key)) games.set(key, twitchId ?? null);
      }
    }
  }
  return games;
}

async function twitchToken(clientId: string, secret: string): Promise<string> {
  const params = new URLSearchParams({
    client_id: clientId,
    client_secret: secret,
    grant_type: "client_credentials",
  });
  const resp = await json<{ access_token: string }>(
    `https://id.twitch.tv/oauth2/token?${params}`,
    { method: "POST" },
  );
  return resp.access_token;
}

function chunks<T>(items: T[], size: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += size)
    out.push(items.slice(i, i + size));
  return out;
}

function labelsFor(igdb: {
  genres?: { name: string }[];
  themes?: { name: string }[];
}): string[] {
  const labels = new Set<string>();
  for (const g of igdb.genres ?? []) {
    const l = GENRE_LABELS[g.name];
    if (l) labels.add(l);
  }
  for (const t of igdb.themes ?? []) {
    const l = THEME_LABELS[t.name];
    if (l) labels.add(l);
  }
  return LABEL_ORDER.filter((l) => labels.has(l));
}

function serialize(mapping: Mapping): string {
  const lines = Object.keys(mapping)
    .sort()
    .map((k) => `  ${JSON.stringify(k)}: ${JSON.stringify(mapping[k])}`);
  return `{\n${lines.join(",\n")}\n}\n`;
}

const refresh = process.argv.includes("--refresh");
const file = Bun.file(OUT);
const existing: Mapping =
  !refresh && (await file.exists()) ? await file.json() : {};

const games = await loadChapterGames();
const todo = [...games].filter(([key]) => !(key in existing));
console.error(`${games.size} games in archive, ${todo.length} to look up`);

const mapping: Mapping = { ...existing };
const unresolved: string[] = [];

for (const [key] of todo) {
  if (NON_GAMING.has(key)) mapping[key] = ["Non-gaming"];
}

const lookups = todo.filter(([key]) => !(key in mapping));
if (lookups.length > 0) {
  const clientId = env("VODKA_TWITCH_CLIENT_ID", "TWITCH_CLIENT_ID");
  const token = await twitchToken(
    clientId,
    env("VODKA_TWITCH_CLIENT_SECRET", "TWITCH_CLIENT_SECRET"),
  );
  const headers = { "Client-Id": clientId, Authorization: `Bearer ${token}` };

  const igdbByTwitch = new Map<string, string>();
  const withId = lookups.filter(([, id]) => id) as [string, string][];
  for (const batch of chunks(withId, 100)) {
    const qs = batch.map(([, id]) => `id=${id}`).join("&");
    const resp = await json<{ data: { id: string; igdb_id: string }[] }>(
      `https://api.twitch.tv/helix/games?${qs}`,
      { headers },
    );
    for (const g of resp.data) if (g.igdb_id) igdbByTwitch.set(g.id, g.igdb_id);
  }

  const igdbIds = [...new Set(igdbByTwitch.values())];
  const igdbGames = new Map<string, Parameters<typeof labelsFor>[0]>();
  for (const batch of chunks(igdbIds, 500)) {
    const resp = await json<
      { id: number; genres?: { name: string }[]; themes?: { name: string }[] }[]
    >("https://api.igdb.com/v4/games", {
      method: "POST",
      headers,
      body: `fields genres.name,themes.name; where id = (${batch.join(",")}); limit 500;`,
    });
    for (const g of resp) igdbGames.set(String(g.id), g);
    await Bun.sleep(300);
  }

  for (const [key, twitchId] of lookups) {
    const igdbId = twitchId ? igdbByTwitch.get(twitchId) : undefined;
    let igdb = igdbId ? igdbGames.get(igdbId) : undefined;
    if (!igdb || labelsFor(igdb).length === 0) {
      const byName = await json<Parameters<typeof labelsFor>[0][]>(
        "https://api.igdb.com/v4/games",
        {
          method: "POST",
          headers,
          body: `fields genres.name,themes.name; where name ~ ${JSON.stringify(key)}; limit 1;`,
        },
      );
      igdb = byName[0];
      await Bun.sleep(300);
    }
    const labels = igdb ? labelsFor(igdb) : [];
    if (labels.length > 0) mapping[key] = labels;
    else unresolved.push(key);
  }
}

await Bun.write(OUT, serialize(mapping));
console.error(
  `wrote ${Object.keys(mapping).length} entries to data/genres.json`,
);
if (unresolved.length > 0) {
  console.error(`\n${unresolved.length} unresolved (tag by hand):`);
  for (const key of unresolved.sort()) console.error(`  ${key}`);
}
