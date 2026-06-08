/* mm-data.jsx — sample content + placeholder art for the Moonmoon mockups.
   Exports to window: MM (data), ArtTile, Thumb, Ic (icons). */

// Deterministic gradient palette for placeholder poster/thumbnail art.
const MM_PALETTES = [
  ['#3a2a6b', '#6c5ce7'], ['#143b4a', '#2fb3d8'], ['#4a1f2e', '#ef4d5b'],
  ['#1f3a2e', '#38c79a'], ['#4a3a16', '#e8c247'], ['#2a1f4a', '#a29bfe'],
  ['#3a1f3a', '#d650b0'], ['#16304a', '#4d8ee8'], ['#3a2616', '#f08a3a'],
  ['#1d3a1f', '#6dcf4a'], ['#2a2a32', '#8b8a96'], ['#0f2f3a', '#25e3d0'],
];
function mmHash(str) {
  let h = 0;
  for (let i = 0; i < str.length; i++) h = (h * 31 + str.charCodeAt(i)) >>> 0;
  return h;
}
function mmPalette(seed) { return MM_PALETTES[mmHash(seed) % MM_PALETTES.length]; }

// Abstract poster/thumbnail placeholder. Reads as intentional key-art, not a
// failed image. `label` text is optional (box art shows the game name).
function ArtTile({ seed, label, showLabel = true, style = {} }) {
  const [a, b] = mmPalette(seed);
  const ang = (mmHash(seed) % 90) + 20;
  const cx = 20 + (mmHash(seed + 'x') % 60);
  const cy = 18 + (mmHash(seed + 'y') % 55);
  return (
    <div className="art" style={{
      position: 'absolute', inset: 0,
      background: `radial-gradient(120% 120% at ${cx}% ${cy}%, ${b} 0%, ${a} 55%, #0b0a12 120%)`,
      ...style,
    }}>
      <div style={{
        position: 'absolute', inset: 0,
        background: `repeating-linear-gradient(${ang}deg, rgba(255,255,255,0.05) 0 2px, transparent 2px 18px)`,
        opacity: 0.5,
      }} />
      <div style={{
        position: 'absolute', width: '52%', height: '52%', left: `${cx - 10}%`, top: `${cy - 8}%`,
        borderRadius: '50%', filter: 'blur(8px)',
        background: `radial-gradient(circle, ${b}cc, transparent 70%)`,
      }} />
      {showLabel && label && (
        <div className="mm-arttext" style={{ fontSize: label.length > 14 ? 15 : 19 }}>{label}</div>
      )}
    </div>
  );
}

// 16:9 thumbnail wrapper used inside VOD cards. children = badges/overlays.
function Thumb({ seed, children }) {
  return (
    <div className="thumb">
      <ArtTile seed={seed} showLabel={false} />
      <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(to top, rgba(8,8,10,0.65), transparent 55%)' }} />
      {children}
    </div>
  );
}

const Ic = {
  dice: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="4" y="4" width="16" height="16" rx="2"/><circle cx="9" cy="9" r="1" fill="currentColor" stroke="none"/><circle cx="15" cy="9" r="1" fill="currentColor" stroke="none"/><circle cx="9" cy="15" r="1" fill="currentColor" stroke="none"/><circle cx="15" cy="15" r="1" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1" fill="currentColor" stroke="none"/></svg>,
  moon: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>,
  sun: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>,
  sync: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 12a9 9 0 1 1-3-6.7"/><polyline points="21 4 21 9 16 9"/></svg>,
  play: <svg viewBox="0 0 24 24"><polygon points="5,3 19,12 5,21"/></svg>,
  arrow: <svg viewBox="0 0 24 14" fill="none"><path d="M1 7h20M16 2l5 5-5 5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/></svg>,
  logo: <svg viewBox="0 0 64 64" aria-hidden="true"><path d="M38 14a20 20 0 1 0 0 36 16 16 0 0 1 0-36z" fill="currentColor"/></svg>,
};

const MM = {
  // the single most-recent VOD (last watched) used by all "Continue" blocks
  resume: {
    title: 'the boat situation has not improved',
    game: 'Elden Ring',
    date: 'Jun 1, 2026',
    duration: '6:42:18',
    at: '2:31:40',
    left: '4h 10m left',
    pct: 38,
  },
  recent: [
    { title: 'we are unfortunately so back', game: 'Just Chatting', date: 'May 31', dur: '4:05:11', pct: 0 },
    { title: 'everything is fine (it is not fine)', game: 'Lethal Company', date: 'May 30', dur: '3:18:44', pct: 72 },
    { title: 'one more boss then bed (a lie)', game: 'Sekiro', date: 'May 28', dur: '5:51:02', pct: 12 },
    { title: 'chat do NOT let me buy the thing', game: 'Old School RuneScape', date: 'May 27', dur: '7:12:09', pct: 0 },
    { title: 'the algorithm chose violence today', game: 'REPO', date: 'May 26', dur: '2:47:33', pct: 0 },
    { title: 'i think the raft is haunted now', game: 'Subnautica', date: 'May 24', dur: '3:55:20', pct: 45 },
  ],
  games: [
    { name: 'Just Chatting', n: 540 }, { name: 'Elden Ring', n: 312 },
    { name: 'Old School RuneScape', n: 134 }, { name: 'Dark Souls III', n: 96 },
    { name: "Baldur's Gate 3", n: 88 }, { name: 'Minecraft', n: 77 },
    { name: 'Lethal Company', n: 58 }, { name: 'Cyberpunk 2077', n: 51 },
    { name: 'Helldivers 2', n: 47 }, { name: 'Sekiro', n: 44 },
    { name: 'Resident Evil 4', n: 33 }, { name: 'REPO', n: 31 },
    { name: 'Schedule I', n: 28 }, { name: 'Liar\u2019s Bar', n: 24 },
    { name: 'Subnautica', n: 22 }, { name: 'Buckshot Roulette', n: 21 },
    { name: 'Content Warning', n: 19 }, { name: 'The Forest', n: 18 },
  ],
  totalGames: 247,
  totalVods: 2841,
};

Object.assign(window, { MM, ArtTile, Thumb, Ic });
