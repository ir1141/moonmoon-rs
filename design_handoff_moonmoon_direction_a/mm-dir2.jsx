/* mm-dir2.jsx — Direction 2: EDITORIAL ARCHIVE
   The library as a publication. Serif masthead, a wide resume banner,
   numbered sections, a ranked games index. Flat, high-contrast, spacious. */

function Dir2() {
  const r = MM.resume;
  return (
    <div className="mm-frame v2">
      <BaseHeader active="Games" />
      <div className="mm-content">
        <div className="v2-masthead">
          <div className="kick">The Moonmoon Archive &middot; Est. 2018</div>
          <div className="vol">{MM.totalVods.toLocaleString()} streams &middot; {MM.totalGames} games</div>
        </div>

        {/* resume banner */}
        <div className="v2-resume">
          <div className="still"><ArtTile seed={r.title} showLabel={false} /></div>
          <div className="copy">
            <div className="kick">Continue watching</div>
            <h2>{r.title}</h2>
            <div className="meta"><b>{r.game}</b> &nbsp;/&nbsp; {r.date} &nbsp;/&nbsp; {r.duration}</div>
            <div className="prog"><i style={{ width: r.pct + '%' }} /></div>
            <div className="meta" style={{ marginBottom: 18 }}>{r.left} &middot; resumes at {r.at}</div>
            <span className="go">Resume stream {Ic.arrow}</span>
          </div>
        </div>

        {/* ranked games index */}
        <div className="v2-secthead">
          <span className="no">01</span>
          <h3>Games index</h3>
          <span className="meta">Ranked by archived streams</span>
        </div>
        <div className="mm-games">
          {MM.games.map((g, i) => (
            <div className="mm-game" key={g.name}>
              <div className="art"><ArtTile seed={g.name} label={g.name} /></div>
              <div className="info">
                <span className="rank">{String(i + 1).padStart(2, '0')}</span>
                <div style={{ minWidth: 0 }}>
                  <div className="name">{g.name}</div>
                  <div className="cnt">{g.n} streams</div>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { Dir2 });
