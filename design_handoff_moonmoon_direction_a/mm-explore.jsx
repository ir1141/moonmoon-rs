/* mm-explore.jsx — focused studies: nav-bar arrangements + chapter treatments.
   Each frame is the refined (Direction A) dark theme + a short rationale caption. */

const EX_I = {
  search: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="11" cy="11" r="7" /><path d="M21 21l-4.3-4.3" /></svg>,
  clock: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 3v5h5" /><path d="M3.05 13A9 9 0 1 0 6 5.3L3 8" /><path d="M12 7v5l3 2" /></svg>,
  sliders: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><line x1="4" y1="8" x2="20" y2="8" /><circle cx="9" cy="8" r="2.6" fill="#1e1e24" /><line x1="4" y1="16" x2="20" y2="16" /><circle cx="15" cy="16" r="2.6" fill="#1e1e24" /></svg>,
};

function Cap({ k, children }) {
  return (
    <div className="ex-peek">
      <div className="ex-cap-k">{k}</div>
      <div className="ex-cap">{children}</div>
    </div>
  );
}

/* ─────────── NAV STUDIES ─────────── */

function NavGrouped() {
  return (
    <div className="mm-frame v1">
      <div className="mm-header" style={{ gap: 18 }}>
        <span className="mm-logo">{Ic.logo}Moonmoon</span>
        <div className="ex-navgroup">
          <a className="active">Games</a><a>Streams</a><a>Calendar</a>
        </div>
        <div className="ex-right">
          <span className="ex-rand">{Ic.dice}Random</span>
          <span className="mm-sep" />
          <span className="ex-link">{EX_I.clock}History</span>
          <span className="ex-iconbtn">{Ic.moon}</span>
          <span className="ex-iconbtn">{Ic.sync}</span>
        </div>
      </div>
      <Cap k="Grouped pills">
        The three <b>browse lenses</b> sit together in one container so they read as views of the
        same archive. <b>History</b> (personal) is pulled to the right, and <b>Random</b> becomes a
        labelled discovery action instead of a mystery icon. Theme &amp; sync stay as quiet utilities.
      </Cap>
    </div>
  );
}

function NavSegmented() {
  return (
    <div className="mm-frame v1">
      <div className="mm-header" style={{ gap: 18 }}>
        <span className="mm-logo">{Ic.logo}Moonmoon</span>
        <span className="ex-seglabel">Browse</span>
        <div className="ex-seg">
          <span className="active">Games</span><span>Streams</span><span>Calendar</span>
        </div>
        <div className="ex-right">
          <span className="ex-iconbtn">{EX_I.search}</span>
          <span className="ex-rand">{Ic.dice}Random</span>
          <span className="mm-sep" />
          <span className="ex-link">{EX_I.clock}History</span>
          <span className="ex-iconbtn">{Ic.moon}</span>
          <span className="ex-iconbtn">{Ic.sync}</span>
        </div>
      </div>
      <Cap k="Segmented “Browse”">
        Makes the “three cuts of one dataset” relationship <b>explicit</b>: one Browse control with a
        clearly selected segment, dropping you from four peer tabs to effectively two destinations
        (Browse / History). Strongest active-state legibility of the three.
      </Cap>
    </div>
  );
}

function NavCentered() {
  return (
    <div className="mm-frame v1">
      <div className="mm-header" style={{ gap: 18 }}>
        <span className="mm-logo">{Ic.logo}Moonmoon</span>
        <div className="ex-center">
          <div className="ex-seg">
            <span className="active">Games</span><span>Streams</span><span>Calendar</span>
          </div>
        </div>
        <div className="ex-right">
          <span className="ex-rand">{Ic.dice}Random</span>
          <span className="ex-iconbtn">{EX_I.sliders}</span>
          <span className="ex-avatar">M</span>
        </div>
      </div>
      <Cap k="Centered + consolidated">
        App-style: browse centered, utilities collapsed. <b>Theme &amp; sync fold into one settings
        control</b>, and <b>History</b> becomes an account-style avatar — the natural home for
        “my watch history / my synced device.” Most opinionated; declutters the bar the most.
      </Cap>
    </div>
  );
}

/* ─────────── CHAPTER STUDIES ─────────── */

const EX_CH = [
  { name: 'Lethal Company', t: '0:00:00', w: 34, c: 5 },
  { name: 'REPO', t: '1:48:20', w: 26, c: 1 },
  { name: 'Subnautica', t: '3:08:55', w: 22, c: 3 },
  { name: 'Just Chatting', t: '4:23:40', w: 18, c: 7 },
];
const EX_TITLE = 'variety night, chat picks the games';

function ChapThumb({ children }) {
  return (
    <div className="thumb">
      <ArtTile seed={EX_TITLE} showLabel={false} />
      <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(to top, rgba(8,8,10,0.65), transparent 55%)' }} />
      {children}
    </div>
  );
}

function ChapBaseline() {
  return (
    <div className="mm-frame v1">
      <div className="ex-card-wrap">
        <div className="mm-vod ex-card">
          <ChapThumb>
            <span className="date">Jun 1</span>
            <span className="dur">5:20:55</span>
            <div className="ex-strip-thin">
              {EX_CH.map((c, i) => <span key={i} style={{ flexBasis: c.w + '%', flexGrow: 0, background: CHAP[c.c] }} />)}
            </div>
          </ChapThumb>
          <div className="body">
            <div className="title">{EX_TITLE}</div>
            <div className="sub">4 games</div>
          </div>
          <div className="ex-disc"><span>Chapters</span><span className="pm">+</span></div>
        </div>
      </div>
      <Cap k="Baseline (today)">
        The strip is 5px and only grows / shows names on <b>hover</b> — invisible on touch — and the
        same chapters are <b>repeated</b> in the “Chapters +” row below, adding chrome and uneven card
        heights.
      </Cap>
    </div>
  );
}

function ChapStripPop() {
  return (
    <div className="mm-frame v1">
      <div className="ex-card-wrap">
        <div className="mm-vod ex-card">
          <ChapThumb>
            <span className="ex-gamechip"><span className="ex-dot" style={{ background: CHAP[5] }} />4 games</span>
            <span className="dur">5:20:55</span>
            <div className="ex-strip-tall">
              {EX_CH.map((c, i) => <span key={i} style={{ flexBasis: c.w + '%', flexGrow: 0, background: CHAP[c.c] }} />)}
            </div>
            <div className="ex-resume-line"><i style={{ width: '24%' }} /></div>
          </ChapThumb>
          <div className="body">
            <div className="title">{EX_TITLE}</div>
            <div className="ex-pop">
              <div className="ph">Jump to a game</div>
              {EX_CH.map((c) => (
                <a key={c.name}>
                  <span className="ex-dot" style={{ background: CHAP[c.c] }} />
                  <span className="nm">{c.name}</span>
                  <span className="tt">{c.t}</span>
                </a>
              ))}
            </div>
          </div>
        </div>
      </div>
      <Cap k="Always-on strip + popover">
        Strip is taller and <b>always visible</b>, labelled by a “4 games” chip. The resume line is a
        separate accent below it. Names live in a <b>popover on tap/click</b> — no permanent duplicate
        list. Recommended.
      </Cap>
    </div>
  );
}

function ChapChips() {
  return (
    <div className="mm-frame v1">
      <div className="ex-card-wrap">
        <div className="mm-vod ex-card">
          <ChapThumb>
            <span className="date">Jun 1</span>
            <span className="dur">5:20:55</span>
            <div className="ex-strip-tall" style={{ bottom: 0 }}>
              {EX_CH.map((c, i) => <span key={i} style={{ flexBasis: c.w + '%', flexGrow: 0, background: CHAP[c.c] }} />)}
            </div>
          </ChapThumb>
          <div className="body">
            <div className="title">{EX_TITLE}</div>
            <div className="ex-chips">
              {EX_CH.slice(0, 3).map((c) => (
                <span key={c.name} className="ex-chip"><span className="ex-dot" style={{ background: CHAP[c.c] }} />{c.name}</span>
              ))}
              <span className="ex-chip more">+1 more</span>
            </div>
          </div>
        </div>
      </div>
      <Cap k="Named chips (touch-first)">
        The biggest chapters surface as <b>real, tappable chips</b> under the title — legible without
        hover and finger-friendly. The thin strip stays purely as the “shape of the stream.”
      </Cap>
    </div>
  );
}

function ChapBars() {
  return (
    <div className="mm-frame v1">
      <div className="ex-card-wrap">
        <div className="mm-vod ex-card">
          <div className="thumb ex-bars-demo">
            <ArtTile seed={EX_TITLE} showLabel={false} />
            <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(to top, rgba(8,8,10,0.7), transparent 60%)' }} />
            <span className="dur">5:20:55</span>
            <div className="ex-seg-tip">Lethal Company · jump</div>
            <div className="ex-strip-tall">
              {EX_CH.map((c, i) => <span key={i} style={{ flexBasis: c.w + '%', flexGrow: 0, background: CHAP[c.c], filter: i === 0 ? 'brightness(1.4)' : 'none' }} />)}
            </div>
            <div className="ex-resume-line"><i style={{ width: '24%' }} /></div>
          </div>
          <div className="body">
            <div className="title">{EX_TITLE}</div>
            <div className="sub">Chapter band &middot; resume line</div>
          </div>
        </div>
      </div>
      <Cap k="Two bars, two meanings">
        Detail of the fix for the stacked-bars smear: a <b>taller, inset, segmented chapter band</b>
        sits above a <b>solid resume accent line</b> with a clear gap — different shape = different
        signal. Hover/active a segment to label + jump.
      </Cap>
    </div>
  );
}

Object.assign(window, {
  NavGrouped, NavSegmented, NavCentered,
  ChapBaseline, ChapStripPop, ChapChips, ChapBars,
});
