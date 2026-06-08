/* mm-a-calendar.jsx — Direction A · Calendar.
   The standout surface, preserved: box-art-filled day cells with glow scaled
   to stream length. Only the chrome is calmed to match Direction A. */

const A_CAL = {
  2: { g: ['Elden Ring'], dur: '5h 12m', glow: 0.6 },
  3: { g: ['Just Chatting'], dur: '3h 40m', glow: 0.3 },
  5: { g: ['Lethal Company', 'REPO'], dur: '6h 02m', glow: 0.8 },
  6: { g: ['Sekiro'], dur: '4h 55m', glow: 0.5 },
  8: { g: ['Old School RuneScape'], dur: '7h 12m', glow: 0.95 },
  9: { g: ['Subnautica'], dur: '3h 18m', glow: 0.25 },
  12: { g: ['Baldur\u2019s Gate 3', 'Minecraft'], dur: '5h 30m', glow: 0.7 },
  13: { g: ['Dark Souls III'], dur: '8h 11m', glow: 1 },
  15: { g: ['Cyberpunk 2077'], dur: '4h 05m', glow: 0.45 },
  16: { g: ['Just Chatting'], dur: '2h 47m', glow: 0.2 },
  19: { g: ['Helldivers 2', 'Content Warning'], dur: '5h 51m', glow: 0.7 },
  20: { g: ['Liar\u2019s Bar'], dur: '3h 33m', glow: 0.35 },
  22: { g: ['Elden Ring'], dur: '6h 20m', glow: 0.85 },
  23: { g: ['Resident Evil 4'], dur: '4h 40m', glow: 0.5 },
  26: { g: ['REPO'], dur: '2h 47m', glow: 0.2 },
  27: { g: ['Old School RuneScape', 'Schedule I'], dur: '7h 12m', glow: 0.95 },
  29: { g: ['Dark Souls III'], dur: '8h 00m', glow: 1 },
  30: { g: ['Sekiro'], dur: '5h 51m', glow: 0.6 },
  31: { g: ['Subnautica'], dur: '3h 55m', glow: 0.4 },
};
const A_CHEV = (d) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" width="17" height="17">
    <path d={d === 'l' ? 'M15 18l-6-6 6-6' : 'M9 18l6-6-6-6'} />
  </svg>
);

function ACalendar() {
  const PAD = 5; // May 2026 starts on a Friday
  const cells = [];
  for (let i = 0; i < PAD; i++) cells.push(<div key={'p' + i} className="a-cal-day pad" />);
  for (let d = 1; d <= 31; d++) {
    const s = A_CAL[d];
    if (!s) {
      cells.push(<div key={d} className="a-cal-day empty"><span className="a-cal-date dim">{d}</span></div>);
      continue;
    }
    const style = {
      borderColor: `rgba(108,92,231,${0.15 + s.glow * 0.5})`,
      boxShadow: `0 0 ${s.glow * 16}px rgba(108,92,231,${s.glow * 0.22})`,
    };
    cells.push(
      <div key={d} className="a-cal-day" style={style}>
        <div className="arts">
          {s.g.map((g) => <div key={g}><ArtTile seed={g} showLabel={false} /></div>)}
        </div>
        <div className="scrim" />
        <span className="a-cal-date">{d}</span>
        <span className="a-cal-dur">{s.dur}</span>
      </div>
    );
  }
  return (
    <div className="mm-frame v1">
      <ANav active="Calendar" />
      <div className="mm-content">
        <div className="a-cal">
          <div className="a-cal-nav">
            <span className="nav">{A_CHEV('l')}</span>
            <h2>May 2026</h2>
            <span className="nav">{A_CHEV('r')}</span>
          </div>
          <div className="a-cal-grid">
            {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map((w) => <div key={w} className="a-cal-wd">{w}</div>)}
            {cells}
          </div>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { ACalendar });
