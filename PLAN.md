# Landing page redesign - remaining work

Working branch: `fix/landing-search-copy` (un-merged, branched from `main`).

Origin: `/impeccable critique` of the landing page, 2026-07-08.
Snapshot at `.impeccable/critique/2026-07-08T10-21-33Z__templates-landing-html.md` (gitignored, local only).
Initial score 21/40 ("Acceptable").

Steps 1-3 are done and committed.
Steps 4-5 are outstanding.
This file is disposable: delete it when the branch merges.

## Done

| Commit | Step | Summary |
|---|---|---|
| `f34f274` | 1. clarify | Search box no longer promises dates/vibes it cannot deliver; `/browse` empty state routes date-seekers to the calendar |
| `7a83a50` | 2. layout | Continue Watching leads the page; hero furniture deleted; CLS 0.0714 -> 0; rail alignment and dead `.rail-vods` CSS |
| `70ff88a` | 3. audit | Focus rings, touch targets, hover gating, `.cal-tg-today-tag` specificity bug |
| `a236204` | 3b. revert | Undid the parts of `70ff88a` that lifted subtle text toward white |

## Step 4 - `/impeccable quieter` - the chapter palette

### 4a. The chapter strip reads as a filled progress bar

`static/css/vods.css:586-609`.
`.chapter-strip` is an 8px, full-width, full-saturation bar at `bottom: 7px`.
Single-chapter VODs produce one segment at `flex-basis: 100%`, which is all four cards in the landing "Recently archived" grid and most of the catalog.
Fallout 4 hashes to `.color-7` = `#d650b0`, hot magenta.
It sits 3px above `.resume-bar` (`--accent`, 3px, `bottom: 0`), which is the *actual* progress indicator.

Why it matters: for an audience whose whole job is "what have I not watched", a solid full-width bar at the bottom of a thumbnail reads as *watched to the end*.
The resting state of the card lies about the one piece of state the user cares most about.

Fix:

1. `.vod-card:has(.chapter-seg:only-child) .chapter-strip { display: none; }` - nothing to segment, no segments.
2. Gate the chip: `templates/vod_card.html:20` -> `{% if show_game_tags && vod.chapter_segments.len() > 1 %}`.
   It currently renders an `aria-haspopup` button reading "1 game" whose popover contains one link to the URL the card already links to twice.
   This also removes 4 tab stops per rail.
3. Desaturate at rest: `.chapter-seg { opacity: .55 }`, full opacity on `:hover` / `:focus-visible`.
4. Give `.resume-bar` a 1px `--bg-card` gap so the two bars are never adjacent.

### 4b. Fold in the last 4 contrast failures

The contrast harness (see "Verification recipe") left exactly 4 AA failures across all 5 routes and both themes, and they share one root cause with 4a.

`.cal-tg-seg.chapter-seg.color-7` on `/calendar` paints white label text (`span.t`, `b`) on `#d650b0` at **3.73:1**, in both themes.

Fix alongside the strip: audit all `chapter-seg` colours so white labels clear 4.5:1, or change the label treatment.
Darkening `color-7` fixes both surfaces at once.

Suggested command: `/impeccable quieter`

## Step 5 - `/impeccable polish`

Minor observations from the critique that are still open.

- `.lp-calhook .cal-txt p` measures 115.4ch at 1440. Cap is 65-75ch; add `max-width: 68ch`.
- Inverted CTAs in `.lp-calhook`: the actual payoff (watch that anniversary VOD) is a 13px inline text link, while the secondary action (open the calendar) is a 196x47px button.
- Two identical calendar SVGs in the same strip, ~1000px apart: `.cal-ic` (decorative) and the one inside `.lp-btn-lg`.
- `.lp-seeall:hover { gap: 11px }` animates a layout property. Use `transform: translateX(3px)` on the `svg` instead.
- No `text-wrap: balance` on any heading.
- `#theme-btn svg { transition: transform 0.4s }` is outside the 150-250ms product band **and** survives `prefers-reduced-motion: reduce` (verified).
- No semantic z-index scale: 0 -> 1 -> 2 -> 3 -> 4 -> 50 -> 100 -> 500, all raw. Ladder them into tokens.
- At 768px the nav pill stretches full-width with `HOME` left-aligned and a lot of dead space: a phone layout applied to a tablet.
- `.duration-badge` and `.game-count-chip` hardcode `rgba(8, 8, 10, 0.8x)`. Worth an `--on-image-scrim` token, alongside the `--on-overlay` / `--on-overlay-dim` / `--on-overlay-accent` family.
- Chips wrap to 3 rows at 390px and 4 rows at 320px, centered and orphaned (2/2/1).
- `Escape` does nothing in the landing search field. There is no way out without Tab.
- `.thumb-link` carries `aria-label="{{ vod.display_title }}"` and `.card-body` contains the same title, so every card reads its title twice to a screen reader.
- `.thumb-link` and `.card-body` are two tab stops pointing at the same `href`.
- The empty catalog is a reachable state (`load_catalog()` -> 120s timeout -> empty, retried every 60s per `EMPTY_RETRY_INTERVAL`).
  It renders two section heads over empty rails and "0 streams across 0 games".
  `.no-results` exists in `base.css:185` and `landing.html` never uses it.
- `/browse` renders "511 games   archived" from `.list-count` + `.list-count-total`. The wording reads oddly.

Suggested command: `/impeccable polish`

## Deferred by decision - not bugs to fix silently

These are real findings that were deliberately not actioned.
Each needs a product call, not a patch.

### Light theme `--border` fails WCAG 1.4.11

`--border: #d6cfbd` measures ~1.34:1 against the surfaces it outlines.
Every card, input and button boundary fails the 3:1 non-text contrast requirement in the light theme.
Fixing it means darkening every outline in light, which changes how heavy the whole theme reads.
That is a design decision, not a bug fix.

### Residual CLS on narrow phones

Below 768px, `.continue-line` may wrap to two lines for a long game name, under-reserving the Continue Watching skeleton by ~16px.
Measured worst case 16.1px at 320px; normal case is -0.1px.
CLS contribution is roughly 0.014, well under the 0.1 "good" threshold.
Forcing a two-line box would add permanent dead space under every short title.

### The pre-paint reserve script duplicates a predicate

`templates/continue_watching_block.html` contains an inline script that must run before the section is parsed, so it cannot import from `static/lib/`.
It re-implements the `selectContinueWatchingEntries` predicate, including the literal `RESUME_MIN_SECONDS = 10`.
If the two drift, the skeleton reserves when it should not (and `continue-watching.js` then collapses it, causing a small upward shift).

Also: `loadHistoryStore` migrates the legacy `moonmoon_resume` key, but the inline script only reads `moonmoon_history`.
A user on the legacy key gets no reservation on their first load after migration, then reserves correctly on every load after.

## Policy learned the hard way

**`PRODUCT.md` sets no formal accessibility target.**
It says: *"No formal target. Keep the existing good habits where they're free."*

Commit `70ff88a` imported WCAG AA from the impeccable skill's generic rules and treated it as a project requirement.
To hit 4.5:1 it lifted `--text-muted` from `#5c5b66` to `#82818d`, which landed 9 RGB units from `--text-dim` and collapsed two type tiers into one colour.
It also promoted four elements a tier and turned the sort value near-white, which was an emphasis change nobody asked for, dressed up as a contrast fix.
`a236204` reverted all of that and settled on `#6f6e7a` (3.99 / 3.60 / 3.31 on `--bg-deep` / `--bg-card` / `--bg-elevated`).

When a skill's generic rule conflicts with the project's stated position, the project wins.
If the project looks wrong, that is a question to ask, not a change to bury in a commit.

## Verification recipe

The server binds `:3000` in production; **never** bind, curl, or browse `:3000`.
Use `PORT=3131 cargo run`.
Stop it by its tracked job id, never by `pkill -f` on the binary name (that pattern matches the production process too).

Before committing Rust changes: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`.
Before committing JS under `static/lib/` or `static/{player,sync}.js`: `bun test`.
Askama compiles templates into the binary, so template edits need a rebuild before they render.

Measurements that earned their keep this round, worth repeating:

- **Contrast harness.** Walk every element with its own text node, resolve the effective background by compositing semi-transparent ancestors, compare against 4.5:1 (or 3:1 for large text). Read `::placeholder` colours out of the CSSOM, since `getComputedStyle` cannot reach them. Run it across all routes and both themes. It found two theming bugs that reading the CSS did not: the `.cal-tg-today-tag` specificity override and the `.stream-badge` theme flip.
- **Focus audit.** Press Tab in a loop and read the computed `outline` of `document.activeElement` at each stop. Flag `outline-style: auto` (the UA ring) and zero-width outlines.
- **Touch targets.** Playwright `browser.newContext({ hasTouch: true, isMobile: true })` to make `@media (pointer: coarse)` and `(hover: none)` actually match. Verify the element did not move: check its right edge against its container, and check the parent paragraph's height did not grow.
- **CLS.** Seed `localStorage.moonmoon_history`, reload, read `performance.getEntriesByType('layout-shift')`. On localhost a fast fetch can beat first paint and give CLS 0 regardless, so **also** measure the skeleton's height against the real card's directly.
- **Suppress transitions before reading colours.** A `transition: color 0.2s` makes `getComputedStyle` return the in-flight value on a theme flip. Inject `* { transition: none !important }` first, or the numbers lie.
- **Screenshots lie too.** A "missing" element was the browser restoring `scrollY`; a "focus ring" was a downscaling artifact. Measure before believing a screenshot.
