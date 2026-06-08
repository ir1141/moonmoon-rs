/* mm-a-history.jsx — Direction A · Watch history.
   Same refined VOD cards, surfacing resume + watched states; client-side. */

const A_HISTORY = [
  { title: 'the boat situation has not improved', game: 'Elden Ring', date: 'Jun 1', dur: '6:42:18', pct: 38 },
  { title: 'everything is fine (it is not fine)', game: 'Lethal Company', date: 'May 30', dur: '3:18:44', pct: 72 },
  { title: 'i think the raft is haunted now', game: 'Subnautica', date: 'May 24', dur: '3:55:20', pct: 45 },
  { title: 'one more boss then bed (a lie)', game: 'Sekiro', date: 'May 28', dur: '5:51:02', pct: 12 },
  { title: 'souls marathon pt. 3 — the wall', game: 'Dark Souls III', date: 'May 29', dur: '8:11:02', watched: true },
  { title: 'chat do NOT let me buy the thing', game: 'Old School RuneScape', date: 'May 27', dur: '7:12:09', watched: true },
];

function AHistory() {
  return (
    <div className="mm-frame v1">
      <ANav active="History" />
      <div className="mm-content">
        <div className="a-pagehead">
          <h1>Watch history</h1>
          <span className="cnt">6 in progress &middot; 2 finished</span>
          <div className="filters">
            <select className="mm-select"><option>Most recently watched</option></select>
          </div>
        </div>
        <div className="a-vodgrid">
          {A_HISTORY.map((v) => <VodWide key={v.title} v={v} />)}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { AHistory });
