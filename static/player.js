(function () {
  'use strict';

  var dataEl = document.getElementById('vod-data');
  if (!dataEl) {
    console.error('[Player] Missing #vod-data element');
    return;
  }
  var VOD_ID = dataEl.dataset.vodId;
  var GAME_HINT = dataEl.dataset.gameHint || '';
  var HAS_EXPLICIT_HINT = GAME_HINT.length > 0;
  var YOUTUBE_IDS;
  try {
    YOUTUBE_IDS = JSON.parse(dataEl.dataset.youtubeIds || '[]');
  } catch (e) {
    console.error('[Player] Failed to parse YouTube IDs:', e);
    YOUTUBE_IDS = [];
  }
  var STORAGE_KEY = 'moonmoon_resume';
  var MAX_RESUME_ENTRIES = 500;
  var MAX_CHAT_DOM_NODES = 2000;

  var MAX_PART_DURATION = 10800; // 3 hours
  var MOONMOON_TWITCH_ID = '121059319';

  var player = null;
  var currentPart = 0;
  // Pre-fill durations: all parts assumed 3hrs, corrected when each loads
  var partDurations = YOUTUBE_IDS.map(function () { return MAX_PART_DURATION; });
  var tickInterval = null;

  // Chat state
  var chatMessages = [];
  var chatIndex = 0;
  var chatCursor = null;
  var chatLoading = false;
  var chatAutoScroll = true;
  var chatRendering = false;
  var chatInitialOffset = 0;
  var lastTickTime = -1;

  // Reply threading: track most recent message per username
  var recentMessageByUser = {};
  var activeReplyPopup = null;
  var REPLY_CHAIN_TIMEOUT = 120; // seconds — 2 min gap breaks the chain
  var REPLY_CHAIN_MAX = 5;

  // Third-party emotes: name → { url, provider }
  var thirdPartyEmotes = {};

  function loadEmotes() {
    // 7TV
    fetch('https://7tv.io/v3/emote-sets/global')
      .then(function (r) { return r.json(); })
      .then(function (data) { parse7TV(data.emotes || []); })
      .catch(function (err) { console.warn('[7TV] Failed to load global emotes:', err); });

    fetch('https://7tv.io/v3/users/twitch/' + MOONMOON_TWITCH_ID)
      .then(function (r) { return r.json(); })
      .then(function (data) {
        if (data.emote_set) parse7TV(data.emote_set.emotes || []);
      })
      .catch(function (err) { console.warn('[7TV] Failed to load channel emotes:', err); });

    // BTTV
    fetch('https://api.betterttv.net/3/cached/emotes/global')
      .then(function (r) { return r.json(); })
      .then(function (emotes) { parseBTTV(emotes); })
      .catch(function (err) { console.warn('[BTTV] Failed to load global emotes:', err); });

    fetch('https://api.betterttv.net/3/cached/users/twitch/' + MOONMOON_TWITCH_ID)
      .then(function (r) { return r.json(); })
      .then(function (data) {
        parseBTTV(data.channelEmotes || []);
        parseBTTV(data.sharedEmotes || []);
      })
      .catch(function (err) { console.warn('[BTTV] Failed to load channel emotes:', err); });

    // FFZ
    fetch('https://api.frankerfacez.com/v1/set/global')
      .then(function (r) { return r.json(); })
      .then(function (data) { parseFFZ(data.sets || {}); })
      .catch(function (err) { console.warn('[FFZ] Failed to load global emotes:', err); });

    fetch('https://api.frankerfacez.com/v1/room/id/' + MOONMOON_TWITCH_ID)
      .then(function (r) { return r.json(); })
      .then(function (data) { parseFFZ(data.sets || {}); })
      .catch(function (err) { console.warn('[FFZ] Failed to load channel emotes:', err); });
  }

  function parse7TV(emotes) {
    for (var i = 0; i < emotes.length; i++) {
      var e = emotes[i];
      var host = e.data && e.data.host;
      if (host && host.url && !thirdPartyEmotes[e.name]) {
        thirdPartyEmotes[e.name] = { url: 'https:' + host.url + '/1x.webp', provider: '7TV' };
      }
    }
  }

  function parseBTTV(emotes) {
    for (var i = 0; i < emotes.length; i++) {
      var e = emotes[i];
      if (e.id && e.code && !thirdPartyEmotes[e.code]) {
        thirdPartyEmotes[e.code] = { url: 'https://cdn.betterttv.net/emote/' + e.id + '/1x', provider: 'BTTV' };
      }
    }
  }

  function parseFFZ(sets) {
    var keys = Object.keys(sets);
    for (var k = 0; k < keys.length; k++) {
      var emotes = sets[keys[k]].emoticons || [];
      for (var i = 0; i < emotes.length; i++) {
        var e = emotes[i];
        if (e.name && e.urls && !thirdPartyEmotes[e.name]) {
          thirdPartyEmotes[e.name] = { url: e.urls['1'] || e.urls['2'] || e.urls['4'], provider: 'FFZ' };
        }
      }
    }
  }

  loadEmotes();

  // DOM refs
  var chatContainer = document.getElementById('chat-messages');
  var chatStatus = document.getElementById('chat-status');
  var chatPaused = document.getElementById('chat-paused');
  var partSelector = document.getElementById('part-selector');

  // ─── Emote Tooltip ───
  var emoteTooltip = document.createElement('div');
  emoteTooltip.className = 'emote-tooltip';
  document.body.appendChild(emoteTooltip);

  chatContainer.addEventListener('mouseover', function (e) {
    var el = e.target;
    if (el.classList.contains('chat-emote') && el.dataset.tooltip) {
      emoteTooltip.textContent = el.dataset.tooltip;
      emoteTooltip.style.display = 'block';
      var rect = el.getBoundingClientRect();
      emoteTooltip.style.left = (rect.left + rect.width / 2 - emoteTooltip.offsetWidth / 2) + 'px';
      emoteTooltip.style.top = (rect.top - emoteTooltip.offsetHeight - 4) + 'px';
    }
  });

  chatContainer.addEventListener('mouseout', function (e) {
    if (e.target.classList.contains('chat-emote')) {
      emoteTooltip.style.display = 'none';
    }
  });

  // ─── Chat Text Size ───

  var CHAT_SIZE_KEY = 'moonmoon_chat_size';
  var chatFontSize = parseInt(localStorage.getItem(CHAT_SIZE_KEY), 10) || 13;
  var MIN_CHAT_SIZE = 10;
  var MAX_CHAT_SIZE = 30;

  function applyChatSize() {
    chatContainer.style.fontSize = chatFontSize + 'px';
    emoteTooltip.style.fontSize = chatFontSize + 'px';
  }

  applyChatSize();

  document.getElementById('chat-size-down').addEventListener('click', function () {
    if (chatFontSize > MIN_CHAT_SIZE) {
      chatFontSize -= 1;
      localStorage.setItem(CHAT_SIZE_KEY, chatFontSize);
      applyChatSize();
    }
  });

  document.getElementById('chat-size-up').addEventListener('click', function () {
    if (chatFontSize < MAX_CHAT_SIZE) {
      chatFontSize += 1;
      localStorage.setItem(CHAT_SIZE_KEY, chatFontSize);
      applyChatSize();
    }
  });

  // ─── Utility ───

  function formatTime(seconds) {
    seconds = Math.max(0, Math.floor(seconds));
    var h = Math.floor(seconds / 3600);
    var m = Math.floor((seconds % 3600) / 60);
    var s = seconds % 60;
    if (h > 0) {
      return h + ':' + String(m).padStart(2, '0') + ':' + String(s).padStart(2, '0');
    }
    return m + ':' + String(s).padStart(2, '0');
  }

  // ─── Multi-Part ───

  function getGlobalTime() {
    var cumulative = 0;
    for (var i = 0; i < currentPart; i++) {
      cumulative += (partDurations[i] || 0);
    }
    var currentTime = 0;
    try {
      currentTime = player && player.getCurrentTime ? player.getCurrentTime() : 0;
    } catch (e) { /* ignore */ }
    return cumulative + currentTime;
  }

  function seekToGlobal(seconds) {
    var cumulative = 0;
    for (var i = 0; i < YOUTUBE_IDS.length; i++) {
      var dur = partDurations[i] || 0;
      if (i === YOUTUBE_IDS.length - 1 || cumulative + dur > seconds) {
        var localTime = seconds - cumulative;
        if (i !== currentPart) {
          switchPart(i, localTime);
        } else {
          player.seekTo(localTime, true);
        }
        return;
      }
      cumulative += dur;
    }
  }

  function clearChatContainer() {
    while (chatContainer.firstChild) {
      chatContainer.removeChild(chatContainer.firstChild);
    }
  }

  function switchPart(index, seekTime) {
    if (index < 0 || index >= YOUTUBE_IDS.length) return;
    currentPart = index;
    lastTickTime = -1;
    chatMessages = [];
    chatIndex = 0;
    chatCursor = null;
    chatLoading = false;
    clearChatContainer();
    recentMessageByUser = {};
    dismissReplyPopup();
    player.loadVideoById(YOUTUBE_IDS[index]);
    if (typeof seekTime === 'number' && seekTime > 0) {
      var seeked = false;
      var waitForPlay = function (e) {
        if (seeked) return;
        if (e.data === YT.PlayerState.PLAYING) {
          seeked = true;
          player.seekTo(seekTime, true);
          player.removeEventListener('onStateChange', waitForPlay);
        }
      };
      player.addEventListener('onStateChange', waitForPlay);
    }
    updatePartSelector();

    // Compute the global time we're switching TO so chat loads aligned with
    // the new playhead. Without this, chat reloads from offset 0 and the
    // tick loop has to page forward through every prior message.
    var cumulative = 0;
    for (var p = 0; p < index; p++) {
      cumulative += (partDurations[p] || 0);
    }
    var localStart = (typeof seekTime === 'number' && seekTime > 0) ? seekTime : 0;
    loadChat(cumulative + localStart);
  }

  function buildPartSelector() {
    if (YOUTUBE_IDS.length <= 1) return;
    while (partSelector.firstChild) {
      partSelector.removeChild(partSelector.firstChild);
    }
    for (var i = 0; i < YOUTUBE_IDS.length; i++) {
      var btn = document.createElement('button');
      btn.className = 'part-btn';
      btn.textContent = 'Part ' + (i + 1);
      btn.dataset.index = i;
      btn.addEventListener('click', function () {
        switchPart(parseInt(this.dataset.index, 10));
      });
      partSelector.appendChild(btn);
    }
    updatePartSelector();
  }

  function updatePartSelector() {
    var buttons = partSelector.querySelectorAll('.part-btn');
    for (var i = 0; i < buttons.length; i++) {
      if (parseInt(buttons[i].dataset.index, 10) === currentPart) {
        buttons[i].classList.add('active');
      } else {
        buttons[i].classList.remove('active');
      }
    }
  }

  // ─── Resume (localStorage) ───

  function getResumeStore() {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY)) || {};
    } catch (e) {
      return {};
    }
  }

  function savePosition() {
    try {
      if (!player || !player.getCurrentTime) return;
      var store = getResumeStore();
      var localTime = 0;
      try {
        localTime = player.getCurrentTime();
      } catch (e) { /* ignore */ }
      store[VOD_ID] = {
        time: getGlobalTime(),
        part: currentPart,
        localTime: localTime,
        updated: Date.now()
      };
      // Enforce max entries
      var keys = Object.keys(store);
      if (keys.length > MAX_RESUME_ENTRIES) {
        keys.sort(function (a, b) {
          return (store[a].updated || 0) - (store[b].updated || 0);
        });
        while (keys.length > MAX_RESUME_ENTRIES) {
          delete store[keys.shift()];
        }
      }
      localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
    } catch (e) { /* quota exceeded or similar */ }
  }

  function getResumePosition() {
    var store = getResumeStore();
    return store[VOD_ID] || null;
  }

  function clearResume() {
    try {
      var store = getResumeStore();
      delete store[VOD_ID];
      localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
    } catch (e) { /* ignore */ }
  }

  // ─── Chat ───

  function loadChat(fromOffset) {
    if (chatLoading) return;
    chatLoading = true;

    var url;
    if (chatCursor && fromOffset === undefined) {
      url = '/api/chat/' + VOD_ID + '?cursor=' + encodeURIComponent(chatCursor);
    } else {
      var offset = (fromOffset !== undefined) ? fromOffset : 0;
      url = '/api/chat/' + VOD_ID + '?content_offset_seconds=' + offset;
      chatInitialOffset = offset;
    }


    fetch(url)
      .then(function (res) {
        if (!res.ok) throw new Error('HTTP ' + res.status);
        return res.json();
      })
      .then(function (data) {
        if (data.comments && data.comments.length > 0) {
          chatMessages = chatMessages.concat(data.comments);
        }
        chatCursor = data.cursor || null;
        chatLoading = false;
      })
      .catch(function (err) {
        console.warn('[Chat] Failed to load:', err);
        chatLoading = false;
        if (chatMessages.length === 0) {
          chatStatus.textContent = 'Chat unavailable';
        }
      });
  }

  function appendTextWithEmotes(parent, text) {
    var words = text.split(' ');
    for (var i = 0; i < words.length; i++) {
      if (i > 0) parent.appendChild(document.createTextNode(' '));
      var emote = thirdPartyEmotes[words[i]];
      if (emote) {
        var img = document.createElement('img');
        img.className = 'chat-emote';
        img.src = emote.url;
        img.alt = words[i];
        img.dataset.tooltip = words[i] + ' [' + emote.provider + ']';
        img.loading = 'lazy';
        parent.appendChild(img);
      } else {
        parent.appendChild(document.createTextNode(words[i]));
      }
    }
  }

  function getReplyTarget(msg) {
    var text = '';
    if (Array.isArray(msg.message)) {
      for (var i = 0; i < msg.message.length; i++) {
        if (msg.message[i].text) { text = msg.message[i].text; break; }
      }
    } else if (msg.message && msg.message.body) {
      text = msg.message.body;
    }
    var match = text.match(/^@(\S+)/);
    return match ? match[1].replace(/[,:.!?]$/, '').toLowerCase() : null;
  }

  function getMessageText(msg) {
    if (Array.isArray(msg.message)) {
      return msg.message.map(function(f) { return f.text || ''; }).join('');
    }
    return (msg.message && msg.message.body) || '';
  }

  function buildReplyChain(node) {
    var chain = [];
    var current = node;
    while (current && current._replyParent && chain.length < REPLY_CHAIN_MAX) {
      chain.unshift(current._replyParent._msgData);
      current = current._replyParent;
    }
    return chain;
  }

  function dismissReplyPopup() {
    if (activeReplyPopup) {
      activeReplyPopup.remove();
      activeReplyPopup = null;
    }
  }

  function showReplyPopup(msgDiv) {
    dismissReplyPopup();

    var chain = buildReplyChain(msgDiv);
    if (chain.length === 0) return;

    var popup = document.createElement('div');
    popup.className = 'reply-popup';

    var header = document.createElement('div');
    header.className = 'reply-popup-header';
    header.textContent = 'Conversation';
    popup.appendChild(header);

    for (var i = 0; i < chain.length; i++) {
      var entry = chain[i];
      var row = document.createElement('div');
      row.className = 'reply-popup-msg';

      var name = document.createElement('span');
      name.className = 'chat-name';
      name.textContent = (entry.display_name || 'Anonymous') + ': ';
      if (entry.user_color && /^#[0-9a-fA-F]{3,8}$/.test(entry.user_color)) {
        name.style.setProperty('--user-color', entry.user_color);
      }
      row.appendChild(name);

      var body = document.createElement('span');
      body.className = 'chat-body';
      appendTextWithEmotes(body, getMessageText(entry));
      row.appendChild(body);

      popup.appendChild(row);
    }

    msgDiv.style.position = 'relative';
    msgDiv.appendChild(popup);
    activeReplyPopup = popup;
  }

  function renderChat() {
    if (!player || !player.getCurrentTime) return;
    var globalTime = getGlobalTime();
    var rendered = false;

    while (chatIndex < chatMessages.length) {
      var msg = chatMessages[chatIndex];
      var offsetSec = parseFloat(msg.content_offset_seconds) || 0;
      if (offsetSec > globalTime) break;

      var div = document.createElement('div');
      div.className = 'chat-msg';

      var nameSpan = document.createElement('span');
      nameSpan.className = 'chat-name';
      nameSpan.textContent = (msg.display_name || 'Anonymous') + ': ';
      if (msg.user_color && /^#[0-9a-fA-F]{3,8}$/.test(msg.user_color)) {
        nameSpan.style.setProperty('--user-color', msg.user_color);
      }

      var bodySpan = document.createElement('span');
      bodySpan.className = 'chat-body';

      if (Array.isArray(msg.message)) {
        for (var fi = 0; fi < msg.message.length; fi++) {
          var frag = msg.message[fi];
          if (frag.emote && frag.emote.emoteID) {
            var img = document.createElement('img');
            img.className = 'chat-emote';
            img.src = 'https://static-cdn.jtvnw.net/emoticons/v2/' + frag.emote.emoteID + '/default/dark/1.0';
            img.alt = frag.text || '';
            img.dataset.tooltip = (frag.text || '') + ' [Twitch]';
            img.loading = 'lazy';
            bodySpan.appendChild(img);
          } else if (frag.text) {
            appendTextWithEmotes(bodySpan, frag.text);
          }
        }
      } else if (msg.message && msg.message.body) {
        appendTextWithEmotes(bodySpan, msg.message.body);
      }

      div.appendChild(nameSpan);
      div.appendChild(bodySpan);

      // Reply threading: store message data and detect @mention chains
      var senderName = (msg.display_name || '').toLowerCase();
      var msgTime = parseFloat(msg.content_offset_seconds) || 0;
      div._msgData = msg;
      div._msgTime = msgTime;

      var replyTarget = getReplyTarget(msg);
      if (replyTarget && recentMessageByUser[replyTarget]) {
        var parentEntry = recentMessageByUser[replyTarget];
        if (Math.abs(msgTime - parentEntry.time) <= REPLY_CHAIN_TIMEOUT) {
          div._replyParent = parentEntry.node;
          var replyBtn = document.createElement('span');
          replyBtn.className = 'chat-reply-btn';
          replyBtn.textContent = '\u21a9';
          replyBtn.title = 'View conversation';
          replyBtn.addEventListener('click', function(e) {
            e.stopPropagation();
            showReplyPopup(this.parentElement);
          });
          div.insertBefore(replyBtn, nameSpan);
        }
      }

      if (senderName) {
        recentMessageByUser[senderName] = { node: div, time: msgTime };
      }

      chatContainer.appendChild(div);

      chatIndex++;
      rendered = true;
    }

    // Prune old DOM nodes to prevent memory leak on long VODs
    while (chatContainer.childNodes.length > MAX_CHAT_DOM_NODES) {
      chatContainer.removeChild(chatContainer.firstChild);
    }

    // Clean up stale reply tracking refs
    for (var user in recentMessageByUser) {
      if (!chatContainer.contains(recentMessageByUser[user].node)) {
        delete recentMessageByUser[user];
      }
    }

    if (rendered && chatAutoScroll) {
      chatRendering = true;
      chatContainer.scrollTop = chatContainer.scrollHeight;
      chatRendering = false;
    }

    // Prefetch: if last rendered message is within 60s of current time and cursor exists
    if (chatCursor && chatIndex > 0 && chatIndex >= chatMessages.length - 1) {
      var lastMsg = chatMessages[chatMessages.length - 1];
      if (lastMsg && (globalTime + 60) >= (parseFloat(lastMsg.content_offset_seconds) || 0)) {
        loadChat();
      }
    }
  }

  // Chat scroll tracking
  chatContainer.addEventListener('scroll', function () {
    if (chatRendering) return;
    dismissReplyPopup();
    var distFromBottom = chatContainer.scrollHeight - chatContainer.scrollTop - chatContainer.clientHeight;
    if (distFromBottom > 100) {
      chatAutoScroll = false;
      chatPaused.style.display = '';
    } else {
      chatAutoScroll = true;
      chatPaused.style.display = 'none';
    }
  });

  chatPaused.addEventListener('click', function () {
    chatAutoScroll = true;
    chatContainer.scrollTop = chatContainer.scrollHeight;
    chatPaused.style.display = 'none';
  });

  chatContainer.addEventListener('click', function(e) {
    if (!e.target.classList.contains('chat-reply-btn')) {
      dismissReplyPopup();
    }
  });

  // ─── Seek detection + chat reset ───

  function resetChat(fromOffset) {
    chatMessages = [];
    chatIndex = 0;
    chatCursor = null;
    chatLoading = false;
    chatAutoScroll = true;
    recentMessageByUser = {};
    dismissReplyPopup();
    clearChatContainer();
    chatPaused.style.display = 'none';
    loadChat(fromOffset);
  }

  // ─── Tick (1 second interval) ───

  function tick() {
    try {
      var state = player.getPlayerState();
      if (state !== YT.PlayerState.PLAYING) return;
    } catch (e) { /* ignore */ }
    var globalTime = getGlobalTime();
    // Detect seek: time jumped by more than 3 seconds
    if (lastTickTime >= 0 && Math.abs(globalTime - lastTickTime) > 3) {
      resetChat(globalTime);
    }
    lastTickTime = globalTime;
    savePosition();
    renderChat();

    // Fallback: YouTube's ENDED event is unreliable in embeds, so also trigger
    // up-next when the final part is within 2s of its duration. Do NOT clear
    // resume here — the real ENDED branch owns that so a mid-window cancel
    // leaves the saved position intact.
    if (!upNextTriggered && currentPart === YOUTUBE_IDS.length - 1) {
      try {
        var dur = player.getDuration();
        var cur = player.getCurrentTime();
        if (dur > 0 && cur >= dur - 2) {
          maybeShowUpNext();
        }
      } catch (e) { /* ignore */ }
    }
  }

  // ─── YouTube IFrame API ───

  window.onYouTubeIframeAPIReady = function () {
    if (!YOUTUBE_IDS || YOUTUBE_IDS.length === 0) {
      var wrap = document.getElementById('player-wrap');
      var notice = document.createElement('div');
      notice.style.cssText = 'display:flex;align-items:center;justify-content:center;height:100%;color:#8b8a96;font-size:16px;';
      notice.textContent = 'No YouTube videos available';
      wrap.appendChild(notice);
      return;
    }

    player = new YT.Player('youtube-player', {
      videoId: YOUTUBE_IDS[0],
      playerVars: {
        autoplay: 1,
        modestbranding: 1,
        rel: 0
      },
      events: {
        onReady: onPlayerReady,
        onStateChange: onPlayerStateChange
      }
    });
  };

  function onPlayerReady() {
    buildPartSelector();

    // Resolve the initial position. Priority: ?t= deep-link > saved resume > start.
    var urlParams = new URLSearchParams(window.location.search);
    var deepLinkT = parseInt(urlParams.get('t'), 10);
    var resume = getResumePosition();
    var initialOffset = 0;
    if (deepLinkT > 0) {
      seekToGlobal(deepLinkT);
      initialOffset = deepLinkT;
    } else if (resume && resume.time > 10) {
      var hasLocal = typeof resume.localTime === 'number' && resume.part != null
        && resume.part >= 0 && resume.part < YOUTUBE_IDS.length;
      if (hasLocal) {
        // Precise resume: jump to the exact part + local offset, sidestepping
        // the partDurations placeholder entirely.
        if (resume.part !== currentPart) {
          switchPart(resume.part, resume.localTime);
          // switchPart already loaded chat with the correct offset (Task 2),
          // and we still need the tick loop running. Skip the boot loadChat below.
          tickInterval = setInterval(tick, 1000);
          return;
        }
        if (resume.localTime > 0) {
          player.seekTo(resume.localTime, true);
        }
        // Same-part resume: at boot currentPart is always 0, so this branch is
        // only reached when resume.part === 0. The loop is therefore a no-op
        // and initialOffset === resume.localTime exactly — no partDurations
        // placeholder leakage.
        var cum = 0;
        for (var pi = 0; pi < resume.part; pi++) cum += (partDurations[pi] || 0);
        initialOffset = cum + resume.localTime;
      } else {
        // Legacy entry (no localTime): fall back to global-time seek.
        seekToGlobal(resume.time);
        initialOffset = resume.time;
      }
    }

    // Load chat from the same offset the player will be at, NOT 0. Otherwise the
    // tick loop pages through every chat message from 0 → initialOffset trying to
    // catch up, which floods the DOM and visibly desyncs from the video.
    loadChat(initialOffset);

    // Start tick
    tickInterval = setInterval(tick, 1000);
  }

  function onPlayerStateChange(event) {
    if (event.data === YT.PlayerState.PLAYING) {
      // Update duration for current part
      try {
        var dur = player.getDuration();
        if (dur > 0) {
          partDurations[currentPart] = dur;
        }
      } catch (e) { /* ignore */ }
    }

    if (event.data === YT.PlayerState.ENDED) {
      // Auto-advance to next part
      if (currentPart < YOUTUBE_IDS.length - 1) {
        switchPart(currentPart + 1);
      } else {
        // All parts finished, clear resume and maybe offer next VOD in the same period
        clearResume();
        maybeShowUpNext();
      }
    }
  }

  // ─── Up Next (auto-continue to next VOD in playing period) ───

  var UP_NEXT_SECONDS = 10;
  var upNextInterval = null;
  var upNextTriggered = false;
  var pendingNextId = null;
  var upNextOverlay = document.getElementById('up-next-overlay');
  var upNextTitleEl = document.getElementById('up-next-title');
  var upNextSecondsEl = document.getElementById('up-next-seconds');
  var upNextCancelBtn = document.getElementById('up-next-cancel');
  var upNextPlayNowBtn = document.getElementById('up-next-play-now');

  if (upNextPlayNowBtn) {
    upNextPlayNowBtn.addEventListener('click', function () {
      if (!pendingNextId) return;
      var id = pendingNextId;
      cancelUpNext();
      navigateToNext(id);
    });
  }
  if (upNextCancelBtn) {
    upNextCancelBtn.addEventListener('click', cancelUpNext);
  }

  function cancelUpNext() {
    if (upNextInterval) {
      clearInterval(upNextInterval);
      upNextInterval = null;
    }
    if (upNextOverlay) upNextOverlay.hidden = true;
  }

  function navigateToNext(nextId) {
    var url = '/watch/' + encodeURIComponent(nextId);
    // Only propagate the game hint if the user explicitly arrived with one —
    // otherwise a single-chapter inference would lock them into a chain.
    if (HAS_EXPLICIT_HINT && GAME_HINT) {
      url += '?game=' + encodeURIComponent(GAME_HINT);
    }
    window.location.href = url;
  }

  function maybeShowUpNext() {
    if (!upNextOverlay) return;
    if (upNextTriggered) return;
    upNextTriggered = true;
    var url = '/api/next/' + encodeURIComponent(VOD_ID);
    if (GAME_HINT) url += '?game=' + encodeURIComponent(GAME_HINT);
    fetch(url)
      .then(function (res) {
        if (res.status === 204) return null;
        if (!res.ok) throw new Error('HTTP ' + res.status);
        return res.json();
      })
      .then(function (data) {
        if (!data || !data.next_id) return;
        pendingNextId = data.next_id;
        upNextTitleEl.textContent = data.next_title || 'Next stream';
        var remaining = UP_NEXT_SECONDS;
        upNextSecondsEl.textContent = remaining;
        upNextOverlay.hidden = false;
        upNextInterval = setInterval(function () {
          remaining -= 1;
          if (remaining <= 0) {
            var id = pendingNextId;
            cancelUpNext();
            navigateToNext(id);
            return;
          }
          upNextSecondsEl.textContent = remaining;
        }, 1000);
      })
      .catch(function (err) {
        console.warn('[UpNext] lookup failed:', err);
      });
  }

  // ─── Save on unload ───

  window.addEventListener('beforeunload', function () {
    savePosition();
    cancelUpNext();
  });

  // ─── Theatre Mode ───

  var THEATRE_KEY = 'moonmoon_theatre';
  var theatreBtn = document.getElementById('theatre-toggle');

  function setTheatre(on) {
    document.body.classList.toggle('theatre-mode', on);
    localStorage.setItem(THEATRE_KEY, on ? '1' : '0');
  }

  // Restore saved preference
  if (localStorage.getItem(THEATRE_KEY) === '1') {
    setTheatre(true);
  }

  theatreBtn.addEventListener('click', function () {
    setTheatre(!document.body.classList.contains('theatre-mode'));
  });

  document.addEventListener('keydown', function (e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    if (e.key === 't' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      setTheatre(!document.body.classList.contains('theatre-mode'));
    }
  });

})();
