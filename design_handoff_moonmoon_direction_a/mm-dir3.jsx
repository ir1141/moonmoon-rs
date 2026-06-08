/* mm-dir3.jsx — Direction 3: NOW PLAYING (broadcast)
   Opens on a cinematic "resume" stage with a glassy scrubber, then flows
   into horizontal rails. Second accent (teal), subtle scanlines, neon edges. */

function RailVod({ v }) {
  return (
    <div className="mm-vod">
      <Thumb seed={v.title}>
        <span className="dur">{v.dur}</span>
        {v.pct > 0 && <div className="rbar"><i style={{ width: v.pct + '%' }} /></div>}
      </Thumb>
      <div className="body">
        <div className="title">{v.title}</div>
        <div className="sub">{v.game} &middot; {v.date}</div>
      </div>
    </div>
  );
}

function Dir3() {
  const r = MM.resume;
  return (
    <div className="mm-frame v3">
      <BaseHeader active="Games" />
      <div className="mm-content">
        {/* cinematic stage */}
        <div className="v3-stage">
          <ArtTile seed={r.title} showLabel={false} />
          <div className="scan" />
          <div className="v3-hero-copy">
            <span className="v3-badge"><i />Last stream &middot; in progress</span>
            <h1>{r.title}</h1>
            <div className="meta"><b>{r.game}</b> &middot; {r.date} &middot; {r.duration}</div>
          </div>
          <div className="v3-controls">
            <div className="play">{Ic.play}</div>
            <div className="scrub">
              <div className="track"><i style={{ width: r.pct + '%' }} /></div>
              <div className="tt"><span>{r.at}</span><span>{r.left}</span></div>
            </div>
            <span className="resume">Resume</span>
          </div>
        </div>

        {/* recently archived rail */}
        <div className="v3-rails" style={{ position: 'relative' }}>
          <div className="v3-railhead">
            <span className="dot" /><h3>Recently archived</h3>
            <span className="more">All streams &rsaquo;</span>
          </div>
          <div className="v3-rail">
            {MM.recent.map((v) => <RailVod key={v.title} v={v} />)}
          </div>
          <div className="v3-fade" style={{ top: 36 }} />
        </div>

        {/* browse games rail */}
        <div className="v3-rails" style={{ position: 'relative', paddingTop: 18 }}>
          <div className="v3-railhead">
            <span className="dot" /><h3>Jump back into</h3>
            <span className="more">{MM.totalGames} games &rsaquo;</span>
          </div>
          <div className="v3-rail" style={{ gridAutoColumns: '152px' }}>
            {MM.games.slice(0, 9).map((g) => (
              <div className="mm-game" key={g.name} style={{ background: '#100e18', borderColor: '#221f30' }}>
                <div style={{ position: 'relative' }}>
                  <div className="art"><ArtTile seed={g.name} label={g.name} /></div>
                  <span className="badge" style={{ color: 'var(--accent2)' }}>{g.n}</span>
                </div>
                <div className="info"><div className="name">{g.name}</div></div>
              </div>
            ))}
          </div>
          <div className="v3-fade" style={{ top: 54 }} />
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { Dir3 });
