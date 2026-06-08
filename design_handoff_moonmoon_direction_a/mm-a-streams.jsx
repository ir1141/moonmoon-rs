/* mm-a-streams.jsx — Direction A · All Streams.
   Wide VOD grid with period headers, chapter strips, resume + watched states. */

const CHAP = ['#ef4d5b', '#f08a3a', '#e8c247', '#6dcf4a', '#38c79a', '#2fb3d8', '#4d8ee8', '#d650b0'];
const Check = <svg viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12" /></svg>;

function VodWide({ v }) {
  const hasCh = !!v.chapters;
  const cls = 'mm-vod' + (v.watched ? ' watched' : '') + (v.pct ? ' has-resume' : '');
  return (
    <div className={cls}>
      <div className="thumb">
        <ArtTile seed={v.title} showLabel={false} />
        <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(to top, rgba(8,8,10,0.65), transparent 55%)' }} />
        {hasCh
          ? <span className="ex-gamechip"><span className="ex-dot" style={{ background: CHAP[v.chapters[0].c] }} />{v.game}</span>
          : <span className="date">{v.date}</span>}
        <span className="dur">{v.dur}</span>
        {v.watched && <span className="watched-badge">{Check}</span>}
        {hasCh && (
          <div className="ex-strip-tall" style={{ bottom: v.pct ? 7 : 0 }}>
            {v.chapters.map((c, i) => <span key={i} style={{ flexBasis: c.w + '%', flexGrow: 0, background: CHAP[c.c] }} />)}
          </div>
        )}
        {v.pct > 0 && (hasCh
          ? <div className="ex-resume-line"><i style={{ width: v.pct + '%' }} /></div>
          : <div className="rbar"><i style={{ width: v.pct + '%' }} /></div>
        )}
      </div>
      <div className="body">
        <div className="title">{v.title}</div>
        <div className="sub">{hasCh ? 'Multi-game stream' : v.game}</div>
      </div>
    </div>
  );
}

const A_STREAMS = {
  june: [
    { title: 'the boat situation has not improved', game: 'Elden Ring', date: 'Jun 1', dur: '6:42:18', pct: 38 },
    { title: 'variety night, chat picks the games', game: '4 games', date: 'Jun 1', dur: '5:20:55', chapters: [{ w: 34, c: 5 }, { w: 26, c: 1 }, { w: 22, c: 3 }, { w: 18, c: 7 }] },
    { title: 'we are unfortunately so back', game: 'Just Chatting', date: 'May 31', dur: '4:05:11' },
  ],
  may: [
    { title: 'everything is fine (it is not fine)', game: 'Lethal Company', date: 'May 30', dur: '3:18:44', pct: 72 },
    { title: 'souls marathon pt. 3 — the wall', game: 'Dark Souls III', date: 'May 29', dur: '8:11:02', watched: true },
    { title: 'one more boss then bed (a lie)', game: 'Sekiro', date: 'May 28', dur: '5:51:02', pct: 12 },
    { title: 'sub-only horror then chatting', game: '2 games', date: 'May 27', dur: '4:47:30', pct: 30, chapters: [{ w: 62, c: 0 }, { w: 38, c: 6 }] },
    { title: 'chat do NOT let me buy the thing', game: 'Old School RuneScape', date: 'May 27', dur: '7:12:09', watched: true },
    { title: 'i think the raft is haunted now', game: 'Subnautica', date: 'May 24', dur: '3:55:20' },
  ],
};

function AStreams() {
  return (
    <div className="mm-frame v1">
      <ANav active="Streams" />
      <div className="mm-content">
        <div className="a-pagehead">
          <h1>All streams</h1>
          <span className="cnt">{MM.totalVods.toLocaleString()} archived</span>
          <div className="filters">
            <input className="mm-input" placeholder="Search streams..." />
            <select className="mm-select"><option>Newest first</option></select>
            <div className="a-daterange">
              <span className="lbl">From</span><span className="a-date">2018-04-01</span>
              <span className="lbl">To</span><span className="a-date">2026-06-01</span>
            </div>
          </div>
        </div>
        <div className="a-vodgrid">
          <div className="a-period">June 2026</div>
          {A_STREAMS.june.map((v) => <VodWide key={v.title} v={v} />)}
          <div className="a-period">May 2026</div>
          {A_STREAMS.may.map((v) => <VodWide key={v.title} v={v} />)}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { AStreams, VodWide, CHAP, Check });
