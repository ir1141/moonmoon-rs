/* mm-a-player.jsx — Direction A · Watch page.
   Video stage + part selector + theatre, with a synced chat-replay panel
   (Twitch-coloured names, emote placeholders). */

const A_MSGS = [
  { ts: '2:31:12', n: 'voidwalker', c: '#a970ff', b: ['LETS GOOO ', { e: '#6dcf4a' }] },
  { ts: '2:31:13', n: 'gigafrog', c: '#00c389', b: ['not the boat again', { e: '#ef4d5b' }] },
  { ts: '2:31:15', n: 'mothmom', c: '#ff4a80', b: ['he is SO close'] },
  { ts: '2:31:16', n: 'crit_happens', c: '#1f9bff', b: [{ e: '#2fb3d8' }, { e: '#2fb3d8' }, ' clip it'] },
  { ts: '2:31:18', n: 'salmonella_jr', c: '#ffb000', b: ['chat he cannot read the rune'] },
  { ts: '2:31:19', n: 'doorknob', c: '#ff6a3d', b: ['the dodge timing is criminal'] },
  { ts: '2:31:21', n: 'tinned_beans', c: '#00b8d4', b: ['actually insane ', { e: '#e8c247' }] },
  { ts: '2:31:22', n: 'pumpkin', c: '#e0457b', b: ['NO WAY'] },
  { ts: '2:31:23', n: 'lurkmaster', c: '#a970ff', b: ['been here 4 hours, worth it'] },
  { ts: '2:31:25', n: 'frogfish', c: '#6dcf4a', b: [{ e: '#a970ff' }, ' the boat is fine'] },
  { ts: '2:31:26', n: 'qwertyuiop', c: '#1f9bff', b: ['second wind LETS GO'] },
  { ts: '2:31:28', n: 'mossback', c: '#ff4a80', b: ['he learned the pattern!!'] },
  { ts: '2:31:29', n: 'gigafrog', c: '#00c389', b: ['ok that was clean ', { e: '#2fb3d8' }] },
  { ts: '2:31:31', n: 'velveteen', c: '#ffb000', b: ['one more try energy'] },
  { ts: '2:31:32', n: 'hexadecimal', c: '#ff6a3d', b: ['the music slaps too'] },
  { ts: '2:31:34', n: 'voidwalker', c: '#a970ff', b: ['CALLED IT ', { e: '#6dcf4a' }, { e: '#6dcf4a' }] },
];

function ChatMsg({ m }) {
  return (
    <div className="a-msg">
      <span className="ts">{m.ts}</span>
      <span className="nm" style={{ color: m.c }}>{m.n}</span>
      <span className="bd">: {m.b.map((t, i) => typeof t === 'string'
        ? <span key={i}>{t}</span>
        : <span key={i} className="a-emote" style={{ background: `linear-gradient(135deg, ${t.e}, ${t.e}88)` }} />)}
      </span>
    </div>
  );
}

function APlayer() {
  const r = MM.resume;
  return (
    <div className="mm-frame v1">
      <ANav active="" />
      <div className="mm-content" style={{ padding: 0, height: 'calc(100% - 60px)', overflow: 'hidden' }}>
        <div className="a-player">
          <div className="a-player-main">
            <div className="a-video">
              <ArtTile seed={r.title} showLabel={false} />
              <div className="scrim" />
              <div className="play">{Ic.play}</div>
              <div className="vbar">
                <div className="track"><i style={{ width: r.pct + '%' }} /></div>
                <div className="tt"><span>{r.at}</span><span>{r.duration}</span></div>
              </div>
            </div>
            <div className="a-pbar">
              <div className="ptitle">{r.title}</div>
              <div className="a-parts">
                <span className="a-part active">Part 1</span>
                <span className="a-part">Part 2</span>
                <span className="a-part">Part 3</span>
              </div>
              <span className="a-theatre">Theatre</span>
            </div>
          </div>
          <div className="a-chat">
            <div className="a-chat-head">
              <span>Chat replay</span>
              <div className="a-chat-ctrl"><span>&minus;</span><span>+</span><span>T</span></div>
            </div>
            <div className="a-chat-msgs">
              {A_MSGS.map((m, i) => <ChatMsg key={i} m={m} />)}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { APlayer, ChatMsg });
