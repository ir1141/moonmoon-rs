# Handoff: moonmoon-rs — Direction A redesign

## Overview
This package specifies a visual redesign of **moonmoon-rs** (the axum + Askama + htmx VOD-archive
browser). It is the result of a design review plus an agreed direction ("**A · Refined Signal**"):
keep the app's existing cosmic identity, but fix hierarchy problems and two specific components.

Three things to implement, in priority order:

1. **Continue Watching → a single "last video" resume hero** (replaces the 4-up shelf). *Highest impact.*
2. **Nav bar → "grouped pills" arrangement** (browse lenses grouped; History + utilities on the right).
3. **Chapters → "always-on labelled strip + popover"** (replaces the hover-only 5px strip *and* the duplicate "Chapters +" disclosure).

Plus a light global pass: calmer glow, typography discipline, counts in the body font.

---

## About the design files
The files in this bundle (`*.html`, `*.jsx`, `moonmoon-mock.css`) are **design references built as a
React/HTML prototype on a pannable canvas**. They are **not** production code to copy.

**Your task is to recreate these designs in the real `moonmoon-rs` codebase**, using its actual stack:
- **Askama templates** (`templates/*.html`) — not React/JSX.
- **Plain CSS**, split per concern (`static/css/*.css`) — the mock's `.mm-frame` / `.v1` / `.ex-*`
  classes are scoping artefacts of the prototype; map their *values* onto the existing class names.
- **htmx** for partials, and the existing vanilla JS modules in `static/`.

The good news: **the prototype's tokens are copied straight from the real app** (`static/css/base.css`),
so colors, fonts and most component styles already match. You're mostly editing markup + a few CSS blocks.

## Fidelity
**High-fidelity.** Colors, typography, spacing, radii and states are final. Recreate pixel-faithfully
using the existing CSS variables. Placeholder "key-art" tiles in the mock stand in for real box art /
thumbnails, which come from the `archive.overpowered.tv` API at runtime exactly as today.

---

## Design tokens (already in `static/css/base.css` — do not redefine)
```
--bg-deep      #08080a     --accent        #6c5ce7
--bg-surface   #111114     --accent-soft   #a29bfe
--bg-card      #16161a     --accent-glow   rgba(108,92,231,0.35)
--bg-elevated  #1e1e24     --text          #e8e6f0
--border       #2a2a32     --text-dim      #8b8a96
                            --text-muted   #5c5b66
Fonts: --font-display "Chakra Petch"  ·  --font-body "Outfit"
```
The mock renames these (`--card`, `--elev`, `--dim`, `--muted`, `--glow`) — they map 1:1 to the
above. **No new tokens or fonts are introduced by Direction A.**

### One global change
Reduce the ambient glow. In `base.css`, the fixed `body::before` radial uses `--accent-glow` at
`0.35`. Direction A dials this to **`rgba(108,92,231,0.18)`** (≈ half). Either lower `--accent-glow`
globally or set the radial to the lower alpha. Keep exactly **one** focal glow (the resume hero / play
button); the per-card hover glows can stay but feel calmer once the ambient wash is reduced.

---

## 1) Continue Watching → single resume hero  ⭐ (the core change)

### Current behaviour
`templates/games.html` renders `#continue-watching` as a **4-card grid** (`continue-grid`), populated
by `static/continue-watching.js` which calls `selectContinueWatchingEntries(store, { limit: 4 })` and
fetches `/history/vods` for those ids. There's also a collapse toggle and a "View history" link.

### Target behaviour
Show **one** card — the single most-recent in-progress VOD — as a horizontal **resume hero**.

**JS (`static/continue-watching.js` + `static/lib/continue-watching.js`):**
- `selectContinueWatchingEntries` already sorts by `updated` descending. **"The last video" is simply
  the first entry.** Change `LIMIT = 4` → `LIMIT = 1` (or pass `{ limit: 1 }`).
- Drop the collapse toggle logic (`initMinimizeToggle`, `COLLAPSED_KEY`) — a single card doesn't need it.
- Render the hero from the one entry's VOD data. You can keep using the existing data shape; just lay
  it out as the hero below instead of a grid card. If easier, fetch the single VOD via `/api/vod/{id}`
  and build the hero, applying resume % from `localStorage` resume state as today.

**Markup (`templates/games.html`):** replace the `continue-shelf` block with a hero:
```
[ 16:9 still | meta column ]
```
- **Container:** `display:flex; gap:20px; padding:16px; border:1px solid var(--border);
  border-radius:14px; background:linear-gradient(120deg, var(--bg-card), #14131c);
  margin-bottom:30px;`
- **Still (left):** `flex:0 0 312px; aspect-ratio:16/9; border-radius:10px; overflow:hidden;
  position:relative;` Thumbnail fills it.
  - **Play button:** centered circle `54×54; border-radius:50%; background:var(--accent);
    box-shadow:0 6px 24px var(--accent-glow);` play glyph `22px`, `fill:#fff; margin-left:3px`.
  - **Resume bar:** bottom of still, `height:4px; background:rgba(0,0,0,0.5);` fill
    `background:var(--accent); width:{resume %}`.
- **Meta (right):** `flex:1; display:flex; flex-direction:column;`
  - Eyebrow "Continue watching": `font-family:var(--font-display); font-size:10px; font-weight:700;
    letter-spacing:1.6px; text-transform:uppercase; color:var(--accent-soft);`
  - Title `<h2>`: `font-family:var(--font-display); font-size:23px; font-weight:700; line-height:1.15;
    margin:8px 0 6px;`
  - Line: `font-size:13px; color:var(--text-dim);` with `<b>` game name in `color:var(--accent-soft);
    font-weight:600;` — pattern: **`<b>{game}</b> · {date} · {duration}`**
  - Sub-line: `margin-top:10px; font-size:12px; color:var(--text-muted);` — "{Xh Ym} left · resumes at {hh:mm:ss}"
  - Actions row: `margin-top:auto; display:flex; gap:10px; padding-top:16px;`
    - **Resume** (primary): `padding:10px 18px; border-radius:9px; background:var(--accent);
      border:1px solid var(--accent); color:#fff; font-family:var(--font-body); font-size:13px;
      font-weight:600;` + play glyph `15px`. Links to `/watch/{id}` (resume position handled by player).
    - **Start over** (secondary): same metrics, `background:var(--bg-elevated);
      border:1px solid var(--border); color:var(--text);` Links to `/watch/{id}` with start=0.

If no in-progress VOD exists, render nothing (as today — the section stays hidden).
Mock reference: `mm-dir1.jsx` (`.v1-resume` in `moonmoon-mock.css`).

### Empty/edge
- 0 resume entries → section hidden.
- Title clamps to 1 line with ellipsis; meta lines never wrap awkwardly.

---

## 2) Nav bar → "grouped pills"

### Current
`templates/base.html` `.header`: logo · four equal `nav-link` pills (Games / Streams / Calendar /
History) · injected `view_title` subtitle · right cluster (random icon, theme, sync). Active = subtle
inset underline on a filled pill.

### Target ordering & grouping (left → right)
```
[Logo]   [ Games · Streams · Calendar ]      …spacer…      [🎲 Random]  |  [↺ History]  [theme]  [sync]
            ^ grouped "browse" container                     action       personal     utilities
```
Rationale: Games/Streams/Calendar are three *lenses on the same archive* → group them. **History is
personal** → move it to the right next to utilities. **Random** becomes a labelled discovery action,
not a bare icon. Theme + Sync remain quiet utilities.

**CSS (extend `static/css/header.css`):**
- **Browse group** (`.nav-group`): `display:flex; align-items:center; gap:2px;
  background:rgba(30,30,36,0.5); border:1px solid var(--border); border-radius:11px; padding:3px;`
  - links: `font-family:var(--font-display); font-size:12px; font-weight:600; letter-spacing:1.6px;
    text-transform:uppercase; color:#8b8a96; padding:7px 14px; border-radius:8px;`
  - **active link:** `color:#fff; background:var(--accent);` (filled — unambiguous; replaces the old
    inset-underline active state).
- **Right cluster** (`.header-right`): `margin-left:auto; display:flex; align-items:center; gap:14px;`
  - **Random** (`.btn-random`): `display:inline-flex; align-items:center; gap:7px;
    border:1px solid color-mix(in oklab, var(--accent) 40%, transparent); border-radius:999px;
    padding:7px 13px; font-family:var(--font-display); font-size:11px; font-weight:600;
    letter-spacing:1.4px; text-transform:uppercase; color:var(--accent-soft);` dice glyph `14px`.
    Links to `/random`.
  - Divider `.header-sep`: `width:1px; height:18px; background:var(--border);`
  - **History** (`.nav-util`): `display:inline-flex; align-items:center; gap:7px;
    font-family:var(--font-display); font-size:12px; font-weight:600; letter-spacing:1.5px;
    text-transform:uppercase; color:#8b8a96;` clock glyph `15px`. On `/history`, active state:
    `color:var(--text);` and glyph `color:var(--accent-soft);`
  - **Theme** & **Sync** icon buttons (`.icon-btn`): `width:34px; height:34px; border-radius:8px;
    border:1px solid transparent; background:transparent; color:var(--text-muted);` glyph `16px`.
    (Demoted vs. today — transparent until hover.)
- **Drop the injected `view_title` subtitle from the header.** Page/context titles already live in the
  body (`vods-hero` heading, player title). This frees the bar and removes the <1024px hide hack.

**Icons:** dice (Random) and theme/sync already exist in `base.html`. Add a **history/clock** glyph for
the History util:
```svg
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M3 3v5h5"/><path d="M3.05 13A9 9 0 1 0 6 5.3L3 8"/><path d="M12 7v5l3 2"/>
</svg>
```
Mock reference: `ANav` in `mm-baseline.jsx` (`.ex-navgroup`, `.ex-rand`, `.ex-link`, `.ex-iconbtn`).

---

## 3) Chapters → always-on labelled strip + popover

This replaces **both** current chapter affordances in `templates/vods_grid.html`:
the hover-only 5px `.chapter-strip` *and* the `<details class="chapter-disclosure">` list.

### Why
The strip is invisible/non-functional on touch (it's gated behind `@media (hover:hover)`), and the
disclosure duplicates the same data on every multi-game card, adding chrome and uneven heights.

### Target (per multi-game VOD card)
1. **"N games" chip** — top-left of the thumb (replaces the date badge on multi-game cards):
   `.game-count-chip`: `position:absolute; top:10px; left:10px; display:inline-flex; align-items:center;
   gap:6px; background:rgba(8,8,10,0.82); backdrop-filter:blur(6px); border-radius:7px; padding:4px 9px;
   font-family:var(--font-display); font-size:11px; font-weight:600; letter-spacing:0.4px;
   color:var(--text);` with a leading `8×8` color dot (first chapter's color).
2. **Always-on chapter band** — taller and always visible (not hover-only):
   `.chapter-strip`: `position:absolute; left:0; right:0; bottom:7px; height:8px; display:flex;
   gap:1.5px; z-index:3;` each segment `flex-basis:{width_pct}%; flex-grow:0; height:100%;
   background:{chapter color}; background-image:linear-gradient(180deg,rgba(255,255,255,.14),rgba(0,0,0,.18));
   border-radius:1px;` Each segment links to its timestamp (`seg.watch_url`) as today.
3. **Separated resume line** — the resume bar sits *below* the band with a clear gap:
   `.resume-bar`: `position:absolute; left:0; right:0; bottom:0; height:3px;
   background:rgba(0,0,0,0.45);` fill `background:var(--accent); box-shadow:0 0 6px var(--accent-glow);`
   The `bottom:7px` on the band leaves a 7px gap so the two bars read as **two distinct signals**
   (chapter band vs. resume), fixing today's stacked-bars smear.
4. **Lift the duration badge** clear of the taller band:
   `.vod-card .duration-badge { bottom: 22px; }` when the card has chapters (today it's `bottom:14px`
   for a 5px strip — needs more for the 8px band). The mock does this with
   `.thumb:has(.ex-strip-tall) .dur { bottom:22px }`.
5. **Names via popover, on demand** — remove the permanent `<details>` disclosure. Replace with a
   tap/click that opens a small popover anchored under the card title (or the chip). Popover:
   `.chapter-pop`: `background:var(--bg-elevated); border:1px solid var(--border); border-radius:10px;
   padding:7px; box-shadow:0 12px 30px rgba(0,0,0,0.45);`
   - header `.chapter-pop-head`: `font-family:var(--font-display); font-size:10px; font-weight:700;
     letter-spacing:1.4px; text-transform:uppercase; color:var(--text-muted); padding:4px 8px 7px;` —
     "Jump to a game".
   - each row (link to timestamp): `display:flex; align-items:center; gap:9px; padding:7px 8px;
     border-radius:7px; color:var(--text-dim); font-size:12.5px;` hover `background:var(--bg-card);`
     contents: `8×8` color dot · name (`flex:1; ellipsis; color:var(--text)`) ·
     timestamp (`font-family:var(--font-display); font-size:11px; color:var(--text-muted);
     font-variant-numeric:tabular-nums;`).
   - **Touch:** tapping a band segment jumps; tapping the chip / a "chapters" affordance opens the
     popover. This is the key fix — names + real tap targets exist without hover.

**Chapter colors** (8-cycle by `color_idx`, already in `vods.css` as `.color-0…7`):
`#ef4d5b #f08a3a #e8c247 #6dcf4a #38c79a #2fb3d8 #4d8ee8 #d650b0`. Color is **decorative rhythm, not a
legend** — don't build UI implying a consistent game↔color mapping.

**JS:** the popover needs a small handler (toggle on click, close on outside click / Esc), reapplied
after `htmx:afterSwap` like other dynamic bits. `static/vod-cards.js` is the natural home.

Mock reference: `ChapStripPop` in `mm-explore.jsx`, and the applied version in `VodWide`
(`mm-a-streams.jsx`). CSS: `.ex-strip-tall`, `.ex-resume-line`, `.ex-gamechip`, `.ex-pop` in
`moonmoon-mock.css`.

---

## Typography discipline (global, low effort, high payoff)
Reserve **Chakra Petch** for true headings, the wordmark, nav, badges and short labels. Stop using it
(uppercase + heavy tracking) for **counts and running metadata** — move those to **Outfit** at normal
tracking. Concretely:
- Stat lines like "247 GAMES archived" → Outfit, e.g. a heading "Games" + muted count "247 archived"
  (`.list-count` / new `.count`: `font-family:var(--font-body); font-size:13px; color:var(--text-dim);
  letter-spacing:0; text-transform:none;`).
- Game-card count badge → make it legible: **"42 VODs"** (Outfit), not a bare number.

---

## Surface-by-surface notes
These reuse the components above; no new patterns.

- **Landing / Games** (`games.html`): resume hero (§1) → toolbar ("Games" heading + muted count +
  search/sort) → games grid (unchanged structurally; badge copy "{n} VODs", count in body font).
- **Streams** (`vods.html` + `vods_grid.html`): page head "All streams" + muted count + search / sort /
  date-range; VOD grid with the chapter treatment (§3), resume + watched states unchanged. Reference:
  `mm-a-streams.jsx`.
- **Calendar** (`calendar.html`): **keep as-is** — it's the strongest surface. Only the header chrome
  changes via §2. Box-art day cells + glow-scaled-to-duration stay. Reference: `mm-a-calendar.jsx`.
- **Player / Watch** (`watch.html`): unchanged layout; header per §2 (no tab active). Reference:
  `mm-a-player.jsx`.
- **History** (`history.html`): unchanged grid; surfaces resume + watched states; header History util
  active (§2). Reference: `mm-a-history.jsx`.

## Shared card values (unchanged from today, for reference)
- `.duration-badge`: `bottom:8px; right:8px; background:rgba(8,8,10,0.88); padding:3px 8px;
  border-radius:6px; font-family:var(--font-display); font-size:11px; font-weight:600;`
- `.date-badge`: `top:8px; left:8px;` same bg/radius, `font-size:10px; color:#b8b6c4;`
- `.watched-badge`: `top:8px; right:8px; 22×22; border-radius:50%;
  background:rgba(108,92,231,0.9);` check glyph `12px; stroke:#fff; stroke-width:2.5;`

---

## Interactions & behaviour
- **Resume hero buttons:** Resume → `/watch/{id}` (player restores position from `localStorage`);
  Start over → `/watch/{id}` from 0.
- **Random:** `/random` (302), as today.
- **Chapter segment:** anchor to `seg.watch_url` (timestamped), as today.
- **Chapter popover:** click/tap toggles; outside-click and Esc close; re-init after `htmx:afterSwap`.
- **Nav active:** server already sets `active_section.slug()` — drive the filled-pill / util-active state from it.
- **Transitions:** keep existing card hover (`transform: translateY`, border + glow). No new animations.
- **Responsive:** below ~768px, resume hero stacks (still on top, meta below); browse group may wrap;
  chapter band stays visible and segments remain tappable (this is the whole point of §3).

## State
No new server state. Client state is unchanged: `moonmoon_resume` (localStorage) drives the resume hero
and progress bars; theme + sync keys as today. The continue-watching JS just selects 1 instead of 4.

## Files in the real codebase to touch
- `templates/base.html` — nav markup (§2), drop `view_title` subtitle.
- `templates/games.html` — resume hero (§1).
- `templates/vods_grid.html` — chapter chip / band / popover (§3).
- `static/css/header.css` — nav group + right cluster (§2).
- `static/css/games.css` — resume hero styles (§1); remove `.continue-shelf` grid styles.
- `static/css/vods.css` — chapter band / resume separation / duration lift / popover (§3).
- `static/css/base.css` — reduce ambient glow; count/label typography.
- `static/continue-watching.js` + `static/lib/continue-watching.js` — `LIMIT 1`, drop collapse, render hero.
- `static/vod-cards.js` — chapter popover handler.

## Design reference files in this bundle
Open **`Moonmoon Redesign.html`** in a browser to explore everything on a pannable canvas. Relevant
sections: **"Direction A · across the app"** (the target), **"Nav bar · arrangements"** (option 1 chosen),
**"Chapters · strip & list"** (option A chosen), and **"Read me first"** (the full review + rationale).
- `moonmoon-mock.css` — all mock styles (search `.v1-resume`, `.ex-navgroup`, `.ex-strip-tall`, `.ex-pop`).
- `mm-baseline.jsx` (`ANav`), `mm-dir1.jsx` (resume hero), `mm-a-streams.jsx` (`VodWide` applied chapters),
  `mm-a-calendar.jsx`, `mm-a-player.jsx`, `mm-a-history.jsx`, `mm-explore.jsx` (nav + chapter studies),
  `mm-data.jsx` (sample data / placeholder art), `design-canvas.jsx` (canvas scaffold — ignore for impl).
