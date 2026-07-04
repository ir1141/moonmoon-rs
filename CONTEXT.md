# Domain glossary

Ubiquitous language for moonmoon-rs. When code, issues, or reviews name one of
these concepts, use the term as defined here — don't drift to the synonyms each
entry lists under _avoid_. Created lazily; extend it as terms get resolved.

## Catalog

One immutable generation of the archive: the `vods` plus everything derived from
them together (`games`, the `CatalogSnapshot`, `date_bounds`). Built only via
`Catalog::build` and swapped atomically, so a reader never sees vods from one
refresh paired with games from another. _Avoid_: dataset, cache (for the whole
generation).

## VOD

One archived stream. An anemic record of mostly-`Option` fields mirroring the
upstream API; its interpretation (stream time, playability, chapters) lives in
free functions today. _Avoid_: video, stream (when you mean the record).

## Lens

The view a list is seen through on `/browse`: the **games lens** (one card per
game) or the **streams lens** (one card per VOD). A non-empty `game` drilldown
forces the streams lens. _Avoid_: view, mode, tab.

## Listing

A page of VOD cards produced by the streams-lens and history pipelines: the
result of taking VOD refs **already in final display order** and running
paginate → build displays → assign headers → nav. Owned by one deep module
(`handlers/listing.rs`, `Listing::build`); callers own only **selection** (the
head that produces the ordered refs — browse filters the catalog, history
resolves client ids). _Avoid_: grid, list, feed (when you mean the built page).

### Period header

The month header ("March 2026") inserted above the first card of each new month
in a chronological Listing. Seeded from the card just before the page slice so a
page starting mid-month doesn't repeat the header. One of the two `Headers`
modes; never combined with a Series header. _Avoid_: month header, date group.

### Series header

The run-length game header ("Elden Ring · 3 streams") inserted above each
contiguous run of the same watched game in a Listing. Requires refs already
ordered so a game forms one run. The other `Headers` mode. _Avoid_: game group,
section header.

## Watch history

The per-VOD record of what the viewer watched. Client-owned: one localStorage
store, `moonmoon_history` (`{ id: { state, time?, updated, part?, localTime? }
}`), whose shape, normalization, legacy migration, merge (per-id last-write-wins
on `updated`, local wins ties) and resume policy all live in one contract
module, `static/lib/history-state.js`. The server touches history in two dumb
roles only: opaque sync transport (`SyncStore`, whole-blob LWW; v2 blob
`{ v, history }`, legacy `{ resume, watched }` blobs readable forever) and
renderer of entries POSTed to `/history/vods` (wire shape pinned on both sides
by `tests/fixtures/history-request.json`). _Avoid_: resume store, watched store
(the pre-unification split).

### Resume policy

"A position at or under 10 seconds is noise" — `RESUME_MIN_SECONDS` plus
`resumePercent`, defined once in `history-state.js` and imported everywhere a
bar is drawn or a resume offered. The server keeps only a positive-position
guard on `/history/resume`; it never re-declares the threshold or the percent.
_Avoid_: duplicating either in Rust or per-caller constants.
