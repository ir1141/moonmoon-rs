/* mm-dir1.jsx — Direction 1: REFINED SIGNAL
   Same cosmic identity, dialed in. Single resume hero, calmer header,
   display face reserved for headings, glow toned down. */

function Dir1() {
  const r = MM.resume;
  return (
    <div className="mm-frame v1">
      <ANav active="Games" />
      <div className="mm-content">
        {/* single Continue Watching hero — the last VOD only */}
        <div className="v1-resume">
          <div className="still">
            <ArtTile seed={r.title} showLabel={false} />
            <div className="play">{Ic.play}</div>
            <div className="rbar"><i style={{ width: r.pct + '%' }} /></div>
          </div>
          <div className="meta">
            <div className="mm-eyebrow" style={{ color: 'var(--accent-soft)' }}>Continue watching</div>
            <h2>{r.title}</h2>
            <div className="line"><b>{r.game}</b> &middot; {r.date} &middot; {r.duration}</div>
            <div className="left">{r.left} &middot; resumes at {r.at}</div>
            <div className="actions">
              <span className="v1-btn primary">{Ic.play} Resume</span>
              <span className="v1-btn">Start over</span>
            </div>
          </div>
        </div>

        {/* toolbar — calmer counts */}
        <div className="mm-toolbar">
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 10 }}>
            <h3 style={{ fontFamily: 'var(--display)', fontSize: 18, letterSpacing: 0.5 }}>Games</h3>
            <span className="mm-count">{MM.totalGames} archived</span>
          </div>
          <div className="mm-filters">
            <input className="mm-input" placeholder="Search games..." />
            <select className="mm-select"><option>Most VODs</option></select>
          </div>
        </div>

        <div className="mm-games">
          {MM.games.map((g) => (
            <div className="mm-game" key={g.name}>
              <div style={{ position: 'relative' }}>
                <div className="art"><ArtTile seed={g.name} label={g.name} /></div>
                <span className="badge">{g.n} VODs</span>
              </div>
              <div className="info"><div className="name">{g.name}</div></div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { Dir1 });
