# Landing page redesign - status

Working branch: `fix/landing-search-copy` (un-merged, branched from `main`).

Origin: `/impeccable critique` of the landing page, 2026-07-08.
Snapshot at `.impeccable/critique/2026-07-08T10-21-33Z__templates-landing-html.md` (gitignored, local only).
Initial score 21/40 ("Acceptable").

**All five steps are done.**
What is left below is the set of findings deliberately *not* actioned, each needing a product call rather than a patch.
This file is disposable: delete it when the branch merges.

## Done

| Commit | Step | Summary |
|---|---|---|
| `f34f274` | 1. clarify | Search box no longer promises dates/vibes it cannot deliver; `/browse` empty state routes date-seekers to the calendar |
| `7a83a50` | 2. layout | Continue Watching leads the page; hero furniture deleted; CLS 0.0714 -> 0; rail alignment and dead `.rail-vods` CSS |
| `70ff88a` | 3. audit | Focus rings, touch targets, hover gating, `.cal-tg-today-tag` specificity bug |
| `a236204` | 3b. revert | Undid the parts of `70ff88a` that lifted subtle text toward white |
| `0ddcaab` | 4. quieter | Chapter strip stops reading as "watched to the end"; calendar timeline darkened so white labels are legible |
| `ae760ac` | 5. polish | Empty catalog state, semantic z-index scale, image-scrim token, motion band, dead ends |

## Deferred by decision - not bugs to fix silently

### Light theme `--border` fails WCAG 1.4.11

`--border: #d6cfbd` measures ~1.34:1 against the surfaces it outlines.
Every card, input and button boundary fails the 3:1 non-text contrast requirement in the light theme.
Fixing it means darkening every outline in light, which changes how heavy the whole theme reads.
That is a design decision, not a bug fix.

### The `.lp-calhook` CTAs are inverted

The actual payoff (watch the anniversary VOD) is a 13px inline text link.
The secondary action (open the calendar) is a 196x47px button.
Left as-is because the block's *purpose* is to advertise `/calendar`; the VOD is the bait, not the destination.
Worth revisiting if the section is ever reframed as "on this day" rather than "calendar hook".

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

### Cards read their title twice to a screen reader

`.thumb-link` carries `aria-label="{{ vod.display_title }}"` and `.card-body` contains the same title.
`thumb-link` is now `tabindex="-1"`, so it is no longer a second tab stop, but it is still in the accessibility tree.
Removing it entirely would also hide the duration badge from the virtual cursor.

### Chip row orphans on small phones

`.lp-chips` wraps to 3 rows at 390px and 4 rows at 320px, centered, leaving a 2/2/1 orphan.
Inspected and judged acceptable; it reads as intentional.

## Policy learned the hard way

**`PRODUCT.md` sets no formal accessibility target.**
It says: *"No formal target. Keep the existing good habits where they're free."*

Commit `70ff88a` imported WCAG AA from the impeccable skill's generic rules and treated it as a project requirement.
To hit 4.5:1 it lifted `--text-muted` from `#5c5b66` to `#82818d`, which landed 9 RGB units from `--text-dim` and collapsed two type tiers into one colour.
It also promoted four elements a tier and turned the sort value near-white, which was an emphasis change nobody asked for, dressed up as a contrast fix.
`a236204` reverted all of that and settled on `#6f6e7a` (3.99 / 3.60 / 3.31 on `--bg-deep` / `--bg-card` / `--bg-elevated`).

When a skill's generic rule conflicts with the project's stated position, the project wins.
If the project looks wrong, that is a question to ask, not a change to bury in a commit.

**Verify before "fixing".**
The critique claimed the 768px nav pill left `HOME` adrift in dead space.
Measured: 4px of slack across a 736px pill, links filling it evenly. Nothing to fix.

## Verification recipe

The server binds `:3000` in production; **never** bind, curl, or browse `:3000`.
Use `PORT=3131 cargo run`.
Stop it by its tracked job id, never by `pkill -f` on the binary name (that pattern matches the production process too).

Before committing Rust changes: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`.
Before committing JS under `static/lib/` or `static/{player,sync}.js`: `bun test`.
Askama compiles templates into the binary, so template edits need a rebuild before they render.

Measurements that earned their keep, worth repeating:

- **Contrast harness.** Walk every element with its own text node, resolve the effective background by compositing semi-transparent ancestors, compare against 4.5:1 (or 3:1 for large text). Read `::placeholder` colours out of the CSSOM, since `getComputedStyle` cannot reach them. Run it across all routes and both themes. It found two theming bugs that reading the CSS did not: the `.cal-tg-today-tag` specificity override and the `.stream-badge` theme flip.
- **`color-mix` reads back as `oklab()`.** Chrome serialises it that way, so a naive rgb regex parses L/a/b as r/g/b and cheerfully reports 20.98:1 for everything. Convert oklab -> linear sRGB -> luminance. Sanity-check the function against white (1.00) and black (21.00) before trusting a single number.
- **Sample the whole palette, not what is on screen.** The audit reported 4 contrast failures on `/calendar`; measuring all eight chapter hues showed every one of them failed. Only `color-7` happened to be rendered that week.
- **Focus audit.** Press Tab in a loop and read the computed `outline` of `document.activeElement` at each stop. Flag `outline-style: auto` (the UA ring) and zero-width outlines.
- **Touch targets.** Playwright `browser.newContext({ hasTouch: true, isMobile: true })` to make `@media (pointer: coarse)` and `(hover: none)` actually match. Verify the element did not move: check its right edge against its container, and check the parent paragraph's height did not grow.
- **CLS.** Seed `localStorage.moonmoon_history`, reload, read `performance.getEntriesByType('layout-shift')`. On localhost a fast fetch can beat first paint and give CLS 0 regardless, so **also** measure the skeleton's height against the real card's directly.
- **Exercise the empty state, don't simulate it.** Point the upstream const at a dead host and boot. A DOM-level simulation would not have caught the chips or the calhook still rendering.
- **Suppress transitions before reading colours.** A `transition: color 0.2s` makes `getComputedStyle` return the in-flight value on a theme flip. Inject `* { transition: none !important }` first, or the numbers lie.
- **Screenshots lie too.** A "missing" element was the browser restoring `scrollY`; a "focus ring" was a downscaling artifact. Measure before believing a screenshot.
