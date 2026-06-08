/* mm-baseline.jsx — faithful recreation of the CURRENT games landing page.
   Serves as the reference the redesign directions are measured against. */

function BaseHeader({ variant = '', active = 'Games' }) {
  const nav = ['Games', 'Streams', 'Calendar', 'History'];
  return (
    <div className="mm-header">
      <span className="mm-logo">{Ic.logo}Moonmoon</span>
      <nav className="mm-nav">
        {nav.map((n) => <a key={n} className={n === active ? 'active' : ''}>{n}</a>)}
      </nav>
      <div className="mm-right">
        <span className="mm-ibtn">{Ic.dice}</span>
        <span className="mm-sep" />
        <span className="mm-ibtn">{Ic.moon}</span>
        <span className="mm-ibtn">{Ic.sync}</span>
      </div>
    </div>
  );
}

function MiniVod({ v }) {
  return (
    <div className="mm-vod">
      <Thumb seed={v.title}>
        <span className="date">{v.date}</span>
        <span className="dur">{v.dur}</span>
        {v.pct > 0 && <div className="rbar"><i style={{ width: v.pct + '%' }} /></div>}
      </Thumb>
      <div className="body">
        <div className="title">{v.title}</div>
        <div className="sub">{v.game}</div>
      </div>
    </div>
  );
}

function Baseline() {
  return (
    <div className="mm-frame">
      <BaseHeader active="Games" />
      <div className="mm-content">
        {/* current 4-up "Continue watching" shelf */}
        <div className="mm-continue">
          <div className="mm-continue-head">
            <div>
              <div className="mm-eyebrow">Pick up where you left off</div>
              <h2>Continue watching</h2>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
              <span className="mm-ibtn" style={{ width: 30, height: 30 }}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ width: 14, height: 14 }}><polyline points="6 9 12 15 18 9" /></svg>
              </span>
              <span className="mm-continue-link">View history</span>
            </div>
          </div>
          <div className="mm-continue-grid">
            <MiniVod v={MM.resume} />
            {MM.recent.slice(0, 3).map((v) => <MiniVod key={v.title} v={v} />)}
          </div>
        </div>

        {/* toolbar */}
        <div className="mm-toolbar">
          <div className="mm-stat">{MM.totalGames} Games <span style={{ color: 'var(--muted)', textTransform: 'none', letterSpacing: 0, fontFamily: 'var(--body)', marginLeft: 4 }}>archived</span></div>
          <div className="mm-filters">
            <input className="mm-input" placeholder="Search games..." />
            <select className="mm-select"><option>Most VODs</option></select>
          </div>
        </div>

        {/* games grid */}
        <div className="mm-games">
          {MM.games.map((g) => (
            <div className="mm-game" key={g.name}>
              <div style={{ position: 'relative' }}>
                <div className="art"><ArtTile seed={g.name} label={g.name} /></div>
                <span className="badge">{g.n}</span>
              </div>
              <div className="info"><div className="name">{g.name}</div></div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

const A_CLOCK = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 3v5h5" /><path d="M3.05 13A9 9 0 1 0 6 5.3L3 8" /><path d="M12 7v5l3 2" />
  </svg>
);

/* Grouped-pills nav (chosen arrangement) — used across all Direction A screens. */
function ANav({ active = 'Games' }) {
  const browse = ['Games', 'Streams', 'Calendar'];
  return (
    <div className="mm-header" style={{ gap: 18 }}>
      <span className="mm-logo">{Ic.logo}Moonmoon</span>
      <div className="ex-navgroup">
        {browse.map((n) => <a key={n} className={active === n ? 'active' : ''}>{n}</a>)}
      </div>
      <div className="ex-right">
        <span className="ex-rand">{Ic.dice}Random</span>
        <span className="mm-sep" />
        <span className={'ex-link' + (active === 'History' ? ' active' : '')}>{A_CLOCK}History</span>
        <span className="ex-iconbtn">{Ic.moon}</span>
        <span className="ex-iconbtn">{Ic.sync}</span>
      </div>
    </div>
  );
}

Object.assign(window, { Baseline, BaseHeader, MiniVod, ANav });
