/* mm-review.jsx — the written design review (the primary deliverable).
   Rendered as a light "document" card so it reads cleanly on the canvas. */

const RV = {
  ink: '#1d1a16', dim: '#5c564d', faint: '#8a8378',
  rule: '#e6e1d6', paper: '#fbf9f4', accent: '#6c5ce7',
  chipBg: '#efeadf',
};

function Sev({ level }) {
  const map = { High: ['#b3372f', '#fbe7e3'], Med: ['#9a6b14', '#f7eddb'], Low: ['#5c564d', '#ece7db'], Keep: ['#2f7d52', '#e1f0e6'] };
  const [c, bg] = map[level] || map.Low;
  return <span style={{ fontSize: 10.5, fontWeight: 700, letterSpacing: 0.6, textTransform: 'uppercase', color: c, background: bg, borderRadius: 5, padding: '3px 8px', flex: '0 0 auto' }}>{level}</span>;
}

function Item({ n, sev, title, obs, rec }) {
  return (
    <div style={{ display: 'flex', gap: 16, padding: '18px 0', borderTop: `1px solid ${RV.rule}` }}>
      <div style={{ fontFamily: '"Chakra Petch", sans-serif', fontSize: 22, fontWeight: 700, color: RV.faint, lineHeight: 1, flex: '0 0 34px' }}>{n}</div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 7 }}>
          <h4 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: RV.ink, letterSpacing: -0.2 }}>{title}</h4>
          <Sev level={sev} />
        </div>
        <p style={{ margin: '0 0 7px', fontSize: 13.5, lineHeight: 1.55, color: RV.dim }}>{obs}</p>
        <p style={{ margin: 0, fontSize: 13.5, lineHeight: 1.55, color: RV.ink }}>
          <b style={{ color: RV.accent }}>Fix &middot; </b>{rec}
        </p>
      </div>
    </div>
  );
}

function Review() {
  return (
    <div style={{ boxSizing: 'border-box', width: '100%', height: '100%', background: RV.paper, fontFamily: '"Outfit", sans-serif', color: RV.ink, padding: '44px 48px', overflow: 'hidden' }}>
      <div style={{ fontFamily: '"Chakra Petch", sans-serif', fontSize: 12, fontWeight: 600, letterSpacing: 2.5, textTransform: 'uppercase', color: RV.accent }}>Design Review</div>
      <h1 style={{ margin: '8px 0 0', fontSize: 34, fontWeight: 700, letterSpacing: -0.6 }}>moonmoon-rs</h1>
      <p style={{ margin: '12px 0 0', fontSize: 15, lineHeight: 1.6, color: RV.dim, maxWidth: '64ch' }}>
        A VOD-archive browser with a real point of view. The bones are strong — a cohesive
        cosmic theme, a delightful calendar, carefully built cards. The opportunities are about
        <i> hierarchy</i>: the page spends its loudest pixels on chrome and metadata, and the
        "Continue watching" shelf has the wrong shape. Below: what to keep, what to change, and
        three directions to compare.
      </p>

      <div style={{ display: 'flex', gap: 24, margin: '26px 0 4px' }}>
        {[['4', 'strengths to keep'], ['7', 'issues flagged'], ['3', 'directions to compare']].map(([k, v]) => (
          <div key={v}>
            <div style={{ fontFamily: '"Chakra Petch", sans-serif', fontSize: 30, fontWeight: 700, color: RV.ink, lineHeight: 1 }}>{k}</div>
            <div style={{ fontSize: 12, color: RV.faint, marginTop: 4, letterSpacing: 0.2 }}>{v}</div>
          </div>
        ))}
      </div>

      {/* strengths */}
      <h3 style={{ margin: '28px 0 10px', fontSize: 13, fontWeight: 700, letterSpacing: 1.5, textTransform: 'uppercase', color: RV.faint }}>What's working — keep it</h3>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px 28px' }}>
        {[
          ['Distinct identity', 'The dark "cosmic" palette, purple accent and Chakra Petch / Outfit pairing give it a clear voice most archive sites never bother with.'],
          ['The calendar', 'Box-art-filled day cells with glow scaled to stream length is genuinely delightful and informative. A signature moment — leave it alone.'],
          ['Considered cards', 'Chapter strips, resume bars, watched states and the hover play affordance show real craft at the component level.'],
          ['Warm light theme', 'The cream light mode is a confident, non-default choice that pairs well with the dark default.'],
        ].map(([t, d]) => (
          <div key={t} style={{ display: 'flex', gap: 10 }}>
            <div style={{ marginTop: 2 }}><Sev level="Keep" /></div>
            <div>
              <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 3 }}>{t}</div>
              <div style={{ fontSize: 12.5, lineHeight: 1.5, color: RV.dim }}>{d}</div>
            </div>
          </div>
        ))}
      </div>

      {/* issues */}
      <h3 style={{ margin: '30px 0 4px', fontSize: 13, fontWeight: 700, letterSpacing: 1.5, textTransform: 'uppercase', color: RV.faint }}>Issues &amp; recommendations</h3>
      <Item n="1" sev="High" title="“Continue watching” is the wrong shape"
        obs="It's framed for resuming where you left off (singular) but renders a four-card grid that duplicates History — with a “View history” link inches away. It out-weights the games grid that is the page's actual job and pushes real content below the fold."
        rec="Collapse it to one resume card — the last VOD — with a clear progress bar and Resume / Start-over. This is exactly the change you asked for, and all three directions below implement it." />
      <Item n="2" sev="Med" title="The header has no hierarchy"
        obs="Logo, four nav pills and three utility icons (random, theme, sync) all carry near-identical weight, and the active pill's inset underline is easy to miss."
        rec="Separate primary navigation from utilities, demote the icon cluster, and make the active state unambiguous." />
      <Item n="3" sev="Med" title="The display face is overworked"
        obs="Chakra Petch in uppercase with heavy tracking drives nav, stats, headings, badges, period headers and eyebrows alike. When everything shouts, nothing leads, and scanning slows."
        rec="Reserve the display face for true headings and the wordmark; let counts and labels fall back to Outfit at normal tracking." />
      <Item n="4" sev="Low" title="Decorative glow is always on"
        obs="The fixed top radial glow plus per-card hover glows and heavy drop shadows accumulate into ambient visual noise."
        rec="Keep one focal glow moment — the resume hero — and calm everything else." />
      <Item n="5" sev="Low" title="The logo mark is cryptic"
        obs="The abstract crescent + “2” glyph doesn't read as “Moonmoon archive,” so it adds confusion rather than recognition."
        rec="Move to a clearer mark or a confident wordmark-only lockup (shown as a placeholder in the mocks)." />
      <Item n="6" sev="Low" title="Metadata is over-styled; content is under-served"
        obs="All-caps tracked “247 GAMES” gives low-value counts heavy texture, while each game card carries only a tiny, unlabelled count badge."
        rec="Quiet the stat lines and make the badge legible — “42 VODs,” not a bare number." />
      <Item n="7" sev="Low" title="Game cards run small and dense"
        obs="150px minimum columns truncate most game titles on the first line."
        rec="Give cards a little more room, or adopt the editorial index treatment (Direction 2) that pairs art with a readable name + count." />

      {/* CW callout */}
      <div style={{ marginTop: 24, padding: '18px 20px', background: '#f1eeff', border: `1px solid #ddd6ff`, borderRadius: 12 }}>
        <div style={{ fontSize: 13, fontWeight: 700, color: RV.accent, marginBottom: 6, letterSpacing: 0.2 }}>On the Continue Watching change</div>
        <p style={{ margin: 0, fontSize: 13.5, lineHeight: 1.6, color: RV.dim }}>
          A single “last video” block is the right call — it matches how the pattern reads everywhere
          else and frees the landing page to do its job. The existing selection logic already sorts
          resume entries by <code style={{ fontFamily: 'ui-monospace, monospace', fontSize: 12.5, background: RV.chipBg, padding: '1px 5px', borderRadius: 4 }}>updated</code>,
          so “the last video” is simply the first entry — effectively a one-line change, plus retiring
          the four-up grid and its collapse toggle.
        </p>
      </div>

      <p style={{ margin: '22px 0 0', fontSize: 13, color: RV.faint, lineHeight: 1.6 }}>
        <b style={{ color: RV.ink }}>Next →</b> Compare the baseline against three directions on the boards to the right.
        Mock imagery is placeholder key-art; real box art and thumbnails come from the archive API at runtime.
      </p>
    </div>
  );
}

Object.assign(window, { Review });
