import "./lib/emote-cache.js";
import {
  computeChatDelay,
  getCachedPartDurations as getCachedPartDurationsPure,
  savePartDuration as savePartDurationPure,
} from "./lib/part-durations.js";
import {
  initialPartDurations,
  parseYoutubePartsDataset,
} from "./lib/player-parts.js";
import {
  formatChatTimestamp,
  isChatTimestampEnabled,
} from "./lib/chat-timestamps.js";
import {
  chatDistanceFromBottom,
  nextChatAutoScrollState,
} from "./lib/chat-autoscroll.js";
import {
  shouldFinalizePlaybackAtTick,
  shouldSaveResume,
} from "./lib/player-completion.js";
import {
  chatEmptyStatusText,
  chatErrorStatusText,
  chatLoadStatusText,
  nextPlayerFallbackState,
  playerFallbackText,
} from "./lib/player-feedback.js";
import { isEmoteCandidate } from "./lib/emote-heuristic.js";
import { fetchChannelEmotes, lookupEmote } from "./lib/emote-client.js";
import { RESUME_KEY as STORAGE_KEY, WATCHED_KEY } from "./lib/history-state.js";
import { markWatchedVod } from "./lib/watched.js";
import { safeLocalStorage, storageGet, storageSet } from "./lib/storage.js";
import { nextChapterPopoverOpen } from "./lib/chapter-popover.js";
import {
  chapterDurationSecs,
  currentChapterIdx,
  formatChapterDuration,
  parseChapters,
} from "./lib/watch-chapters.js";

var dataEl = document.getElementById("vod-data");
if (!dataEl) {
  throw new Error("[Player] Missing #vod-data element");
}

// localStorage access throws SecurityError in storage-blocking browsers; a
// bare module-eval call would abort the whole player module, so all access
// goes through the lib/storage.js guards against this handle.
var storage = safeLocalStorage();
var VOD_ID = dataEl.dataset.vodId || "";
var GAME_HINT = dataEl.dataset.gameHint || "";
var HAS_EXPLICIT_HINT = GAME_HINT.length > 0;
var YOUTUBE_PARTS = parseYoutubePartsDataset(
  dataEl.dataset.youtubeParts || "",
  dataEl.dataset.youtubeIds || "",
);
var YOUTUBE_IDS = YOUTUBE_PARTS.map(function (part) {
  return part.id;
});
var CHAPTERS = parseChapters(dataEl.dataset.chapters || "");
var VOD_TOTAL_SECS = parseInt(dataEl.dataset.totalSecs || "0", 10) || 0;
var MAX_PART_DURATION = 10800; // 3 hours
// Twitch chat offsets run on the original broadcast clock, but the YouTube
// re-uploads can be shorter than the broadcast (parts are capped at 3h and the
// capture can miss content). This constant gap maps player time → chat time so
// chat stays aligned. Mirrors the upstream site's `delay`. Unknown part
// durations are estimated at the 3h cap, matching the playback timeline.
var CHAT_DELAY = computeChatDelay(VOD_TOTAL_SECS, YOUTUBE_PARTS, MAX_PART_DURATION);
var MAX_RESUME_ENTRIES = 500;
var MAX_WATCHED_ENTRIES = 500;
var PART_DURATIONS_KEY = "moonmoon_part_durations";
var MAX_PART_DURATION_ENTRIES = 500;
var MAX_CHAT_DOM_NODES = 2000;
var CHAT_SCROLL_INTENT_MS = 800;

var player = null;
var currentPart = 0;
var watchCompleted = false;
var partDurations = initialPartDurations(
  YOUTUBE_PARTS,
  null,
  MAX_PART_DURATION,
);
var tickInterval = null;

// Chat state
var chatMessages = [];
var chatIndex = 0;
var chatCursor = null;
var chatLoading = false;
var chatAutoScroll = true;
var chatRendering = false;
var chatInitialOffset = 0;
var chatUserScrollIntentUntil = 0;
var chatAutoScrollFrame = null;
var chatScrollGeneration = 0;
var lastTickTime = -1;
// Bumped whenever chat state is reset (seek, part switch). In-flight fetches
// capture the value at start and drop their response if it has moved on.
var chatGeneration = 0;

// Reply threading: track most recent message per username
var recentMessageByUser = {};
var activeReplyPopup = null;
var REPLY_CHAIN_TIMEOUT = 120; // seconds — 2 min gap breaks the chain
var REPLY_CHAIN_MAX = 5;

// Third-party emotes: name → { url, provider }
var thirdPartyEmotes = {};

function loadEmotes() {
  fetchChannelEmotes().then(function (emotes) {
    var names = Object.keys(emotes);
    for (var i = 0; i < names.length; i++) {
      if (!thirdPartyEmotes[names[i]]) {
        thirdPartyEmotes[names[i]] = emotes[names[i]];
      }
    }
  });
}

// Per-VOD emote snapshot. The archive froze the exact emote set active during
// this stream (newer VODs only); loading it up front turns most chat words
// into local hits and preserves since-removed / collision-prone emotes with
// their stream-time ids. Snapshot entries OVERWRITE prefetched ones — see the
// gap-fill guard in loadEmotes, so the snapshot always wins on overlap
// regardless of which fetch resolves first. Old VODs / failures return an empty
// map and fall through to prefetch + lazy lookup.
function loadVodEmotes() {
  if (!VOD_ID) return;
  fetch("/api/emotes/vod/" + encodeURIComponent(VOD_ID))
    .then(function (res) {
      return res.ok ? res.json() : { emotes: {} };
    })
    .then(function (body) {
      var emotes = body.emotes || {};
      var names = Object.keys(emotes);
      for (var i = 0; i < names.length; i++) {
        thirdPartyEmotes[names[i]] = emotes[names[i]];
      }
    })
    .catch(function (err) {
      console.warn("[Emote] vod snapshot fetch failed:", err);
    });
}

// ─── Lazy lookup ───
// Emotes from non-Moonmoon sets (e.g. TANIMURA, owned by another channel)
// won't appear in the prefetched channel map above. When we see an unknown
// word that *looks* like an emote, ask the server once via lookupEmote. Hits
// go into thirdPartyEmotes; misses go into emoteMissCache so we never re-query
// a known non-emote. Transient failures are left unrecorded so they retry.

var emoteMissCache = Object.create(null);
var pendingEmoteLookups = Object.create(null);

function lazyResolveEmote(name, textNode) {
  if (!isEmoteCandidate(name)) return;
  if (thirdPartyEmotes[name]) {
    swapTextNodeForEmote(textNode, name, thirdPartyEmotes[name]);
    return;
  }
  if (emoteMissCache[name]) return;

  if (pendingEmoteLookups[name]) {
    pendingEmoteLookups[name].then(
      function (record) {
        applyResolved(name, record, textNode);
      },
      function () {},
    );
    return;
  }

  var p = lookupEmote(name);
  pendingEmoteLookups[name] = p;
  p.then(
    function (record) {
      delete pendingEmoteLookups[name];
      if (record && record.hit) {
        thirdPartyEmotes[name] = {
          url: record.url,
          provider: record.provider,
          owner: record.owner,
        };
        applyResolved(name, record, textNode);
      } else if (record && !record.transient) {
        emoteMissCache[name] = true;
      }
    },
    function () {
      delete pendingEmoteLookups[name];
    },
  );
}

function applyResolved(name, record, textNode) {
  if (!record || !record.hit) return;
  swapTextNodeForEmote(textNode, name, {
    url: record.url,
    provider: record.provider,
    owner: record.owner,
  });
}

function buildEmoteImg(name, emote) {
  var img = document.createElement("img");
  img.className = "chat-emote";
  img.src = emote.url;
  img.alt = name;
  img.dataset.tooltip = formatEmoteTooltip(name, emote);
  img.loading = "lazy";
  trackChatImageLoad(img);
  return img;
}

function swapTextNodeForEmote(textNode, name, emote) {
  if (!textNode || !textNode.parentNode) return;
  if (textNode.nodeType !== Node.TEXT_NODE) return;
  if (textNode.nodeValue !== name) return; // text was edited or already swapped
  textNode.parentNode.replaceChild(buildEmoteImg(name, emote), textNode);
  scheduleChatAutoScroll();
}

function formatEmoteTooltip(name, emote) {
  var label = "[" + emote.provider;
  if (emote.owner) label += " · " + emote.owner;
  label += "]";
  return name + " " + label;
}

loadEmotes();
loadVodEmotes();

// DOM refs
var chatContainer = document.getElementById("chat-messages");
var chatStatus = document.getElementById("chat-status");
var chatRetry = document.getElementById("chat-retry");
var chatPaused = document.getElementById("chat-paused");
var chatTimestampToggle = document.getElementById("chat-timestamp-toggle");
var partSelector = document.getElementById("part-selector");
var chapterSelector = document.getElementById("chapter-selector");
var playerWrap = document.getElementById("player-wrap");
var playerFallback = document.getElementById("player-fallback");
var youtubePlayerElement = document.getElementById("youtube-player");
var youtubeReady = false;
var playerFallbackState = {
  shown: false,
  playerHidden: false,
  reason: null,
};
var playerFallbackNotice = null;
var chatRetryOffset;
var PLAYER_INIT_TIMEOUT_MS = 8000;
var playerInitTimeout = null;

function chatNow() {
  return window.performance && typeof window.performance.now === "function"
    ? window.performance.now()
    : Date.now();
}

function markChatUserScrollIntent() {
  chatUserScrollIntentUntil = chatNow() + CHAT_SCROLL_INTENT_MS;
}

function hasChatUserScrollIntent() {
  return chatNow() <= chatUserScrollIntentUntil;
}

function chatScrollMeasurement() {
  return {
    scrollHeight: chatContainer.scrollHeight,
    scrollTop: chatContainer.scrollTop,
    clientHeight: chatContainer.clientHeight,
  };
}

function setChatPausedVisible(paused) {
  chatPaused.style.display = paused ? "" : "none";
}

function scrollChatToBottom() {
  var generation = ++chatScrollGeneration;
  chatRendering = true;
  chatContainer.scrollTop = chatContainer.scrollHeight;

  window.requestAnimationFrame(function () {
    chatContainer.scrollTop = chatContainer.scrollHeight;
    if (generation === chatScrollGeneration) {
      chatRendering = false;
    }
  });
}

function scheduleChatAutoScroll() {
  if (!chatAutoScroll || chatAutoScrollFrame !== null) return;

  chatAutoScrollFrame = window.requestAnimationFrame(function () {
    chatAutoScrollFrame = null;
    if (chatAutoScroll) scrollChatToBottom();
  });
}

function trackChatImageLoad(img) {
  img.addEventListener("load", scheduleChatAutoScroll, { once: true });
  if (img.complete) scheduleChatAutoScroll();
}

// ─── Emote Tooltip ───
var emoteTooltip = document.createElement("div");
emoteTooltip.className = "emote-tooltip";
document.body.appendChild(emoteTooltip);

function dismissTransientChatUi() {
  dismissReplyPopup();
  emoteTooltip.style.display = "none";
}

document.body.addEventListener("htmx:beforeSwap", dismissTransientChatUi);

function setChatStatus(text) {
  if (chatStatus) chatStatus.textContent = text;
}

function setChatRetryVisible(visible) {
  if (chatRetry) chatRetry.hidden = !visible;
}

if (chatRetry) {
  chatRetry.addEventListener("click", function () {
    setChatRetryVisible(false);
    loadChat(chatRetryOffset);
  });
}

chatContainer.addEventListener("mouseover", function (e) {
  var el = e.target;
  if (!(el instanceof HTMLElement)) return;
  if (el.classList.contains("chat-emote") && el.dataset.tooltip) {
    emoteTooltip.textContent = el.dataset.tooltip;
    emoteTooltip.style.display = "block";
    var rect = el.getBoundingClientRect();
    emoteTooltip.style.left =
      rect.left + rect.width / 2 - emoteTooltip.offsetWidth / 2 + "px";
    emoteTooltip.style.top = rect.top - emoteTooltip.offsetHeight - 4 + "px";
  }
});

chatContainer.addEventListener("mouseout", function (e) {
  if (!(e.target instanceof HTMLElement)) return;
  if (e.target.classList.contains("chat-emote")) {
    emoteTooltip.style.display = "none";
  }
});

// ─── Chat Text Size ───

var CHAT_SIZE_KEY = "moonmoon_chat_size";
var chatFontSize = parseInt(storageGet(storage, CHAT_SIZE_KEY), 10) || 13;
var MIN_CHAT_SIZE = 10;
var MAX_CHAT_SIZE = 30;
var CHAT_TIMESTAMPS_KEY = "moonmoon_chat_timestamps";
var chatTimestampsEnabled = isChatTimestampEnabled(
  storageGet(storage, CHAT_TIMESTAMPS_KEY),
);

function applyChatSize() {
  var stickToBottom = chatAutoScroll;
  chatContainer.style.fontSize = chatFontSize + "px";
  emoteTooltip.style.fontSize = chatFontSize + "px";
  if (stickToBottom) {
    scrollChatToBottom();
  }
}

applyChatSize();

document
  .getElementById("chat-size-down")
  .addEventListener("click", function () {
    if (chatFontSize > MIN_CHAT_SIZE) {
      chatFontSize -= 1;
      storageSet(storage, CHAT_SIZE_KEY, String(chatFontSize));
      applyChatSize();
    }
  });

document.getElementById("chat-size-up").addEventListener("click", function () {
  if (chatFontSize < MAX_CHAT_SIZE) {
    chatFontSize += 1;
    storageSet(storage, CHAT_SIZE_KEY, String(chatFontSize));
    applyChatSize();
  }
});

function applyChatTimestamps() {
  chatContainer.classList.toggle("show-timestamps", chatTimestampsEnabled);
  chatTimestampToggle.classList.toggle("active", chatTimestampsEnabled);
  chatTimestampToggle.setAttribute(
    "aria-pressed",
    chatTimestampsEnabled ? "true" : "false",
  );
  chatTimestampToggle.title = chatTimestampsEnabled
    ? "Hide timestamps"
    : "Show timestamps";
}

applyChatTimestamps();

chatTimestampToggle.addEventListener("click", function () {
  chatTimestampsEnabled = !chatTimestampsEnabled;
  storageSet(storage, CHAT_TIMESTAMPS_KEY, String(chatTimestampsEnabled));
  applyChatTimestamps();
});

// ─── Utility ───

function formatTime(seconds) {
  seconds = Math.max(0, Math.floor(seconds));
  var h = Math.floor(seconds / 3600);
  var m = Math.floor((seconds % 3600) / 60);
  var s = seconds % 60;
  if (h > 0) {
    return (
      h + ":" + String(m).padStart(2, "0") + ":" + String(s).padStart(2, "0")
    );
  }
  return m + ":" + String(s).padStart(2, "0");
}

// ─── Multi-Part ───

function getGlobalTime() {
  var cumulative = 0;
  for (var i = 0; i < currentPart; i++) {
    cumulative += partDurations[i] || 0;
  }
  var currentTime = 0;
  try {
    currentTime = player && player.getCurrentTime ? player.getCurrentTime() : 0;
  } catch (e) {
    /* ignore */
  }
  return cumulative + currentTime;
}

// Player time mapped onto the Twitch broadcast clock that chat offsets use.
// Resume/seek stay in player time; only chat fetching and rendering use this.
function getChatTime() {
  return getGlobalTime() + CHAT_DELAY;
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

var pendingSeekListener = null;

function clearPendingSeekListener() {
  if (!pendingSeekListener) return;
  try {
    player.removeEventListener("onStateChange", pendingSeekListener);
  } catch (e) {
    /* ignore — the expectedPart guard below defuses it anyway */
  }
  pendingSeekListener = null;
}

function switchPart(index, seekTime) {
  if (index < 0 || index >= YOUTUBE_IDS.length) return;
  clearPendingSeekListener();
  chatGeneration += 1;
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
  if (typeof seekTime === "number" && seekTime > 0) {
    var seeked = false;
    var expectedPart = index;
    var waitForPlay = function (e) {
      if (seeked) return;
      if (currentPart !== expectedPart) {
        // a later switchPart superseded this seek
        seeked = true;
        player.removeEventListener("onStateChange", waitForPlay);
        if (pendingSeekListener === waitForPlay) pendingSeekListener = null;
        return;
      }
      if (e.data === YT.PlayerState.PLAYING) {
        seeked = true;
        player.seekTo(seekTime, true);
        player.removeEventListener("onStateChange", waitForPlay);
        if (pendingSeekListener === waitForPlay) pendingSeekListener = null;
      }
    };
    pendingSeekListener = waitForPlay;
    player.addEventListener("onStateChange", waitForPlay);
  }
  updatePartSelector();

  // Compute the global time we're switching TO so chat loads aligned with
  // the new playhead. Without this, chat reloads from offset 0 and the
  // tick loop has to page forward through every prior message.
  var cumulative = 0;
  for (var p = 0; p < index; p++) {
    cumulative += partDurations[p] || 0;
  }
  var localStart = typeof seekTime === "number" && seekTime > 0 ? seekTime : 0;
  loadChat(cumulative + localStart + CHAT_DELAY);
}

function buildPartSelector() {
  // The chapter selector replaces the parts picker when the VOD has chapters to
  // navigate by; fall back to the Part N buttons only when it doesn't. Part
  // switching itself still works through seekToGlobal/switchPart either way.
  if (CHAPTERS.length > 1) {
    partSelector.hidden = true;
    return;
  }
  if (YOUTUBE_IDS.length <= 1) return;
  while (partSelector.firstChild) {
    partSelector.removeChild(partSelector.firstChild);
  }
  for (var i = 0; i < YOUTUBE_IDS.length; i++) {
    var btn = document.createElement("button");
    btn.className = "btn-chip part-btn";
    btn.textContent = "Part " + (i + 1);
    btn.dataset.index = String(i);
    btn.addEventListener("click", function () {
      var button = /** @type {HTMLButtonElement} */ (this);
      switchPart(parseInt(button.dataset.index || "0", 10));
    });
    partSelector.appendChild(btn);
  }
  updatePartSelector();
}

function updatePartSelector() {
  var buttons = /** @type {NodeListOf<HTMLButtonElement>} */ (
    partSelector.querySelectorAll(".part-btn")
  );
  for (var i = 0; i < buttons.length; i++) {
    if (parseInt(buttons[i].dataset.index, 10) === currentPart) {
      buttons[i].classList.add("active");
    } else {
      buttons[i].classList.remove("active");
    }
  }
}

// ─── Chapter selector (game chip + "jump to a game" popover) ───

var chapterPopoverOpen = false;
var currentChapterIndex = -1;
var chapterChip = null;
var chapterChipDot = null;
var chapterChipLabel = null;
var chapterPopover = null;
var chapterLinks = [];

function buildChapterCaret() {
  var ns = "http://www.w3.org/2000/svg";
  var svg = document.createElementNS(ns, "svg");
  svg.setAttribute("class", "wc-caret");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2.5");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  var poly = document.createElementNS(ns, "polyline");
  poly.setAttribute("points", "6 9 12 15 18 9");
  svg.appendChild(poly);
  return svg;
}

// Total stream length, used only for the final chapter's duration label. Prefer
// the VOD's authoritative duration (the same value the server clamps chapter
// offsets to); summing YouTube part durations would over-count whenever a part's
// duration is missing server-side and falls back to the MAX_PART_DURATION
// placeholder. When the chip renders there is always >1 chapter, which only
// happens when the server duration is > 0.
function totalTimelineSecs() {
  if (VOD_TOTAL_SECS > 0) return VOD_TOTAL_SECS;
  return CHAPTERS.length ? CHAPTERS[CHAPTERS.length - 1].start : 0;
}

function setChapterPopoverOpen(open) {
  if (!chapterChip || !chapterPopover) return;
  chapterPopoverOpen = open;
  chapterChip.setAttribute("aria-expanded", open ? "true" : "false");
  chapterPopover.hidden = !open;
}

// Recompute the current chapter from the live playhead and reflect it in the chip
// (label + dot color) and the popover's active row. DOM writes are gated on an
// index change so this stays cheap at 1 Hz.
function updateActiveChapter() {
  if (!chapterChip || CHAPTERS.length <= 1) return;
  var idx = currentChapterIdx(CHAPTERS, getGlobalTime());
  if (idx === currentChapterIndex) return;
  currentChapterIndex = idx;
  var cur = CHAPTERS[idx];
  chapterChipLabel.textContent = cur.name;
  chapterChipDot.className = "wc-chip-dot color-" + cur.color;
  for (var i = 0; i < chapterLinks.length; i++) {
    chapterLinks[i].classList.toggle("active", i === idx);
  }
}

function buildChapterSelector() {
  if (!chapterSelector || CHAPTERS.length <= 1) return;
  var total = totalTimelineSecs();

  var chip = document.createElement("button");
  chip.type = "button";
  chip.className = "btn-chip wc-chapters-chip";
  chip.setAttribute("aria-haspopup", "true");
  chip.setAttribute("aria-expanded", "false");
  chip.setAttribute("aria-label", "Jump to a game");

  var dot = document.createElement("span");
  dot.className = "wc-chip-dot";
  var label = document.createElement("span");
  label.className = "wc-chip-label";
  chip.appendChild(dot);
  chip.appendChild(label);
  chip.appendChild(buildChapterCaret());

  var pop = document.createElement("div");
  pop.className = "wc-pop";
  pop.setAttribute("role", "menu");
  pop.hidden = true;

  var head = document.createElement("div");
  head.className = "wc-pop-head";
  var headTitle = document.createElement("span");
  headTitle.textContent = "Jump to a game";
  var headCount = document.createElement("span");
  headCount.className = "wc-pop-count";
  headCount.textContent = CHAPTERS.length + " games";
  head.appendChild(headTitle);
  head.appendChild(headCount);
  pop.appendChild(head);

  chapterLinks = [];
  for (var i = 0; i < CHAPTERS.length; i++) {
    var c = CHAPTERS[i];
    var link = document.createElement("button");
    link.type = "button";
    link.className = "wc-pop-link";
    link.setAttribute("role", "menuitem");
    link.dataset.index = String(i);

    var marker = document.createElement("span");
    marker.className = "wc-pop-marker color-" + c.color;
    marker.setAttribute("aria-hidden", "true");

    var name = document.createElement("span");
    name.className = "wc-pop-name";
    name.textContent = c.name;

    var meta = document.createElement("span");
    meta.className = "wc-pop-meta";
    var timeEl = document.createElement("span");
    timeEl.className = "wc-pop-time";
    timeEl.textContent = formatTime(c.start);
    var nowEl = document.createElement("span");
    nowEl.className = "wc-pop-nowtag";
    nowEl.textContent = "Now";
    var durEl = document.createElement("span");
    durEl.className = "wc-pop-dur";
    durEl.textContent = formatChapterDuration(
      chapterDurationSecs(CHAPTERS, i, total),
    );
    meta.appendChild(timeEl);
    meta.appendChild(nowEl);
    meta.appendChild(durEl);

    link.appendChild(marker);
    link.appendChild(name);
    link.appendChild(meta);
    link.addEventListener("click", function () {
      var button = /** @type {HTMLButtonElement} */ (this);
      var idx = parseInt(button.dataset.index || "0", 10);
      seekToGlobal(CHAPTERS[idx].start);
      setChapterPopoverOpen(false);
    });

    pop.appendChild(link);
    chapterLinks.push(link);
  }

  chip.addEventListener("click", function (e) {
    e.preventDefault();
    e.stopPropagation();
    setChapterPopoverOpen(
      nextChapterPopoverOpen(chapterPopoverOpen, { type: "chip" }),
    );
  });

  chapterSelector.appendChild(chip);
  chapterSelector.appendChild(pop);

  chapterChip = chip;
  chapterChipDot = dot;
  chapterChipLabel = label;
  chapterPopover = pop;

  // Close on outside pointer-down and Escape, mirroring the card popover wiring.
  document.addEventListener("mousedown", function (e) {
    if (!chapterPopoverOpen) return;
    if (e.target instanceof Node && chapterSelector.contains(e.target)) return;
    setChapterPopoverOpen(
      nextChapterPopoverOpen(chapterPopoverOpen, { type: "outside" }),
    );
  });
  document.addEventListener("keydown", function (e) {
    if (e.key !== "Escape" || !chapterPopoverOpen) return;
    setChapterPopoverOpen(
      nextChapterPopoverOpen(chapterPopoverOpen, { type: "escape" }),
    );
    chapterChip.focus();
  });

  updateActiveChapter();
}

// ─── Part duration cache (localStorage) ───
// Cache real YouTube part durations as we observe them, so multi-part resumes
// can align chat against actual durations instead of MAX_PART_DURATION
// placeholders. Sentinel: 0 means "unknown for this index".

function getPartDurationsStore() {
  try {
    return JSON.parse(storageGet(storage, PART_DURATIONS_KEY)) || {};
  } catch (e) {
    return {};
  }
}

function getCachedPartDurations() {
  return getCachedPartDurationsPure(
    getPartDurationsStore(),
    VOD_ID,
    YOUTUBE_IDS.length,
  );
}

function savePartDuration(index, duration) {
  var store = getPartDurationsStore();
  var next = savePartDurationPure(
    store,
    VOD_ID,
    YOUTUBE_IDS.length,
    index,
    duration,
    MAX_PART_DURATION_ENTRIES,
    Date.now(),
  );
  if (next === store) return;
  try {
    storageSet(storage, PART_DURATIONS_KEY, JSON.stringify(next));
  } catch (e) {
    /* quota exceeded or similar */
  }
}

// ─── Resume (localStorage) ───

// In-memory cache of the parsed resume store. savePosition runs at 1 Hz; a
// fresh JSON.parse of up to 500 entries every tick is measurable jank.
// Invalidated when another writer (sync.js merge, another tab) touches the key.
var resumeStoreCache = null;

function getResumeStore() {
  if (resumeStoreCache) return resumeStoreCache;
  try {
    resumeStoreCache = JSON.parse(storageGet(storage, STORAGE_KEY)) || {};
  } catch (e) {
    resumeStoreCache = {};
  }
  return resumeStoreCache;
}

window.addEventListener("storage", function (e) {
  if (e.key === STORAGE_KEY) resumeStoreCache = null;
});
window.addEventListener("moonmoon:resumeChanged", function () {
  resumeStoreCache = null;
});

function savePosition() {
  if (!shouldSaveResume({ completed: watchCompleted })) return;

  try {
    if (!player || !player.getCurrentTime) return;
    var store = getResumeStore();
    var localTime = 0;
    try {
      localTime = player.getCurrentTime();
    } catch (e) {
      /* ignore */
    }
    store[VOD_ID] = {
      time: getGlobalTime(),
      part: currentPart,
      localTime: localTime,
      updated: Date.now(),
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
    storageSet(storage, STORAGE_KEY, JSON.stringify(store));
  } catch (e) {
    /* quota exceeded or similar */
  }
}

function getResumePosition() {
  var store = getResumeStore();
  return store[VOD_ID] || null;
}

function clearResume() {
  try {
    var store = getResumeStore();
    delete store[VOD_ID];
    storageSet(storage, STORAGE_KEY, JSON.stringify(store));
  } catch (e) {
    /* ignore */
  }
}

function getWatchedStore() {
  try {
    return JSON.parse(storageGet(storage, WATCHED_KEY)) || {};
  } catch (e) {
    return {};
  }
}

function markWatched() {
  try {
    var next = markWatchedVod(
      getWatchedStore(),
      VOD_ID,
      Date.now(),
      MAX_WATCHED_ENTRIES,
    );
    storageSet(storage, WATCHED_KEY, JSON.stringify(next));
    window.dispatchEvent(new Event("moonmoon:watchedChanged"));
  } catch (e) {
    /* quota exceeded or similar */
  }
}

function finalizeCurrentVod() {
  if (watchCompleted) return;
  watchCompleted = true;
  clearResume();
  markWatched();
}

// ─── Chat ───

function loadChat(fromOffset) {
  if (chatLoading) return;
  chatLoading = true;
  var gen = chatGeneration;
  chatRetryOffset = fromOffset;
  setChatRetryVisible(false);
  setChatStatus(chatLoadStatusText());

  var url;
  if (chatCursor && fromOffset === undefined) {
    url = "/api/chat/" + VOD_ID + "?cursor=" + encodeURIComponent(chatCursor);
  } else {
    var offset = fromOffset !== undefined ? fromOffset : 0;
    url = "/api/chat/" + VOD_ID + "?content_offset_seconds=" + offset;
    chatInitialOffset = offset;
  }

  fetch(url)
    .then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    })
    .then(function (data) {
      if (gen !== chatGeneration) return; // stale: chat state was reset mid-flight
      if (data.comments && data.comments.length > 0) {
        chatMessages = chatMessages.concat(data.comments);
      }
      chatCursor = data.cursor || null;
      chatLoading = false;
      if (chatMessages.length === 0) {
        setChatStatus(chatEmptyStatusText());
      } else {
        setChatStatus("");
      }
    })
    .catch(function (err) {
      if (gen !== chatGeneration) return;
      console.warn("[Chat] Failed to load:", err);
      chatLoading = false;
      setChatStatus(chatErrorStatusText());
      setChatRetryVisible(true);
    });
}

function appendTextWithEmotes(parent, text) {
  var words = text.split(" ");
  for (var i = 0; i < words.length; i++) {
    if (i > 0) parent.appendChild(document.createTextNode(" "));
    var word = words[i];
    var emote = thirdPartyEmotes[word];
    if (emote) {
      parent.appendChild(buildEmoteImg(word, emote));
    } else {
      var textNode = document.createTextNode(word);
      parent.appendChild(textNode);
      lazyResolveEmote(word, textNode);
    }
  }
}

function getReplyTarget(msg) {
  var text = "";
  if (Array.isArray(msg.message)) {
    for (var i = 0; i < msg.message.length; i++) {
      if (msg.message[i].text) {
        text = msg.message[i].text;
        break;
      }
    }
  } else if (msg.message && msg.message.body) {
    text = msg.message.body;
  }
  var match = text.match(/^@(\S+)/);
  return match ? match[1].replace(/[,:.!?]$/, "").toLowerCase() : null;
}

function getMessageText(msg) {
  if (Array.isArray(msg.message)) {
    return msg.message
      .map(function (f) {
        return f.text || "";
      })
      .join("");
  }
  return (msg.message && msg.message.body) || "";
}

function buildChatTimestamp(msg) {
  var timestamp = document.createElement("span");
  timestamp.className = "chat-timestamp";
  timestamp.textContent = formatChatTimestamp(msg.content_offset_seconds);
  return timestamp;
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

  var popup = document.createElement("div");
  popup.className = "reply-popup";

  var header = document.createElement("div");
  header.className = "reply-popup-header";
  header.textContent = "Conversation";
  popup.appendChild(header);

  for (var i = 0; i < chain.length; i++) {
    var entry = chain[i];
    var row = document.createElement("div");
    row.className = "reply-popup-msg";

    row.appendChild(buildChatTimestamp(entry));

    var name = document.createElement("span");
    name.className = "chat-name";
    name.textContent = (entry.display_name || "Anonymous") + ": ";
    if (entry.user_color && /^#[0-9a-fA-F]{3,8}$/.test(entry.user_color)) {
      name.style.setProperty("--user-color", entry.user_color);
    }
    row.appendChild(name);

    var body = document.createElement("span");
    body.className = "chat-body";
    appendTextWithEmotes(body, getMessageText(entry));
    row.appendChild(body);

    popup.appendChild(row);
  }

  msgDiv.style.position = "relative";
  msgDiv.appendChild(popup);
  activeReplyPopup = popup;
}

function renderChat() {
  if (!player || !player.getCurrentTime) return;
  var globalTime = getChatTime();
  var rendered = false;

  while (chatIndex < chatMessages.length) {
    var msg = chatMessages[chatIndex];
    var offsetSec = parseFloat(msg.content_offset_seconds) || 0;
    if (offsetSec > globalTime) break;

    var div = document.createElement("div");
    div.className = "chat-msg";

    div.appendChild(buildChatTimestamp(msg));

    var nameSpan = document.createElement("span");
    nameSpan.className = "chat-name";
    nameSpan.textContent = (msg.display_name || "Anonymous") + ": ";
    if (msg.user_color && /^#[0-9a-fA-F]{3,8}$/.test(msg.user_color)) {
      nameSpan.style.setProperty("--user-color", msg.user_color);
    }

    var bodySpan = document.createElement("span");
    bodySpan.className = "chat-body";

    if (Array.isArray(msg.message)) {
      for (var fi = 0; fi < msg.message.length; fi++) {
        var frag = msg.message[fi];
        if (frag.emote && frag.emote.emoteID) {
          var img = document.createElement("img");
          img.className = "chat-emote";
          img.src =
            "https://static-cdn.jtvnw.net/emoticons/v2/" +
            frag.emote.emoteID +
            "/default/dark/1.0";
          img.alt = frag.text || "";
          img.dataset.tooltip = (frag.text || "") + " [Twitch]";
          img.loading = "lazy";
          trackChatImageLoad(img);
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
    var senderName = (msg.display_name || "").toLowerCase();
    var msgTime = parseFloat(msg.content_offset_seconds) || 0;
    div._msgData = msg;
    div._msgTime = msgTime;

    var replyTarget = getReplyTarget(msg);
    if (replyTarget && recentMessageByUser[replyTarget]) {
      var parentEntry = recentMessageByUser[replyTarget];
      if (Math.abs(msgTime - parentEntry.time) <= REPLY_CHAIN_TIMEOUT) {
        div._replyParent = parentEntry.node;
        var replyBtn = document.createElement("span");
        replyBtn.className = "chat-reply-btn";
        replyBtn.textContent = "\u21a9";
        replyBtn.title = "View conversation";
        replyBtn.addEventListener("click", function (e) {
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
    scrollChatToBottom();
  }

  // Prefetch: if last rendered message is within 60s of current time and cursor exists
  if (chatCursor && chatIndex > 0 && chatIndex >= chatMessages.length - 1) {
    var lastMsg = chatMessages[chatMessages.length - 1];
    if (
      lastMsg &&
      globalTime + 60 >= (parseFloat(lastMsg.content_offset_seconds) || 0)
    ) {
      loadChat();
    }
  }
}

// Chat scroll tracking
chatContainer.addEventListener("wheel", markChatUserScrollIntent, {
  passive: true,
});
chatContainer.addEventListener("touchmove", markChatUserScrollIntent, {
  passive: true,
});
chatContainer.addEventListener("keydown", function (e) {
  if (
    e.key === "ArrowUp" ||
    e.key === "ArrowDown" ||
    e.key === "PageUp" ||
    e.key === "PageDown" ||
    e.key === "Home" ||
    e.key === "End" ||
    e.key === " "
  ) {
    markChatUserScrollIntent();
  }
});
chatContainer.addEventListener("pointerdown", function (e) {
  var rect = chatContainer.getBoundingClientRect();
  if (e.clientX >= rect.right - 18) markChatUserScrollIntent();
});

chatContainer.addEventListener("scroll", function () {
  var userInitiated = hasChatUserScrollIntent();
  if (chatRendering && !userInitiated) return;
  if (userInitiated) dismissReplyPopup();

  var measurement = chatScrollMeasurement();
  var nextState = nextChatAutoScrollState(measurement, {
    currentAutoScroll: chatAutoScroll,
    userInitiated: userInitiated,
  });

  chatAutoScroll = nextState.autoScroll;
  setChatPausedVisible(nextState.paused);

  if (
    chatAutoScroll &&
    !userInitiated &&
    chatDistanceFromBottom(measurement) > 0
  ) {
    scheduleChatAutoScroll();
  }
});

chatPaused.addEventListener("click", function () {
  chatAutoScroll = true;
  scrollChatToBottom();
  setChatPausedVisible(false);
});

chatContainer.addEventListener("click", function (e) {
  if (
    !(e.target instanceof HTMLElement) ||
    !e.target.classList.contains("chat-reply-btn")
  ) {
    dismissReplyPopup();
  }
});

// ─── Seek detection + chat reset ───

function resetChat(fromOffset) {
  chatGeneration += 1;
  chatMessages = [];
  chatIndex = 0;
  chatCursor = null;
  chatLoading = false;
  chatAutoScroll = true;
  recentMessageByUser = {};
  dismissReplyPopup();
  clearChatContainer();
  chatPaused.style.display = "none";
  setChatRetryVisible(false);
  loadChat(fromOffset);
}

// ─── Tick (1 second interval) ───

function tick() {
  // Keep the chapter chip in sync even while paused (e.g. autoplay blocked after
  // a deep-link seek), so this runs before the PLAYING guard below.
  updateActiveChapter();
  try {
    var state = player.getPlayerState();
    if (state !== YT.PlayerState.PLAYING) return;
  } catch (e) {
    /* ignore */
  }
  var globalTime = getChatTime();
  // Detect seek: time jumped by more than 3 seconds. Also treat the very first
  // tick after PLAYING as a potential seek if globalTime is non-trivial — this
  // catches the autoplay-blocked path where the resume seek happened during
  // UNSTARTED (which the tick guard skips), so we never observed the jump.
  var jumped = lastTickTime >= 0 && Math.abs(globalTime - lastTickTime) > 3;
  var firstTickAtOffset =
    lastTickTime < 0 &&
    globalTime > 10 &&
    chatMessages.length === 0 &&
    !chatLoading;
  if (jumped || firstTickAtOffset) {
    resetChat(globalTime);
  }
  lastTickTime = globalTime;
  savePosition();
  renderChat();

  // Fallback: YouTube's ENDED event is unreliable in embeds, so treat the
  // final part's last seconds as completed for watched/resume state too.
  if (!upNextTriggered && currentPart === YOUTUBE_IDS.length - 1) {
    try {
      var dur = player.getDuration();
      var cur = player.getCurrentTime();
      if (
        shouldFinalizePlaybackAtTick({
          currentPart: currentPart,
          partCount: YOUTUBE_IDS.length,
          duration: dur,
          currentTime: cur,
        })
      ) {
        finalizeCurrentVod();
        maybeShowUpNext();
      }
    } catch (e) {
      /* ignore */
    }
  }
}

// ─── YouTube IFrame API ───

function showPlayerFallback(reason) {
  if (playerFallbackState.shown) return;
  playerFallbackState = nextPlayerFallbackState(playerFallbackState, {
    type: "show",
    reason: reason,
  });

  if (youtubePlayerElement) {
    youtubePlayerElement.hidden = playerFallbackState.playerHidden;
  }
  if (playerFallback) {
    playerFallback.textContent = playerFallbackText(reason);
    playerFallback.hidden = false;
    return;
  }

  if (playerWrap) {
    var notice = document.createElement("div");
    notice.className = "player-fallback";
    notice.textContent = playerFallbackText(reason);
    playerWrap.appendChild(notice);
    playerFallbackNotice = notice;
  }
}

function clearPlayerFallback() {
  playerFallbackState = nextPlayerFallbackState(playerFallbackState, {
    type: "player-ready",
  });
  if (youtubePlayerElement) youtubePlayerElement.hidden = false;
  if (playerFallback) {
    playerFallback.textContent = "";
    playerFallback.hidden = true;
  }
  if (playerFallbackNotice) {
    playerFallbackNotice.remove();
    playerFallbackNotice = null;
  }
}

function clearPlayerInitTimeout() {
  if (!playerInitTimeout) return;
  window.clearTimeout(playerInitTimeout);
  playerInitTimeout = null;
}

window.onYouTubeIframeAPIReady = function () {
  youtubeReady = true;
  clearPlayerInitTimeout();
  if (!YOUTUBE_IDS || YOUTUBE_IDS.length === 0) {
    showPlayerFallback("missing-video");
    return;
  }

  try {
    player = new YT.Player("youtube-player", {
      videoId: YOUTUBE_IDS[0],
      playerVars: {
        autoplay: 1,
        modestbranding: 1,
        rel: 0,
      },
      events: {
        onReady: onPlayerReady,
        onStateChange: onPlayerStateChange,
        onError: onPlayerError,
      },
    });
  } catch (err) {
    console.warn("[Player] YouTube player failed to initialize:", err);
    showPlayerFallback("api-failed");
  }
};

if (!YOUTUBE_IDS || YOUTUBE_IDS.length === 0) {
  showPlayerFallback("missing-video");
} else {
  playerInitTimeout = window.setTimeout(function () {
    playerInitTimeout = null;
    if (!youtubeReady && !player) showPlayerFallback("api-failed");
  }, PLAYER_INIT_TIMEOUT_MS);
}

// player.js is a module (deferred). The YT iframe API may have already fired
// its ready callback before this script ran — in that case our newly-assigned
// onYouTubeIframeAPIReady will never be invoked. If YT is already loaded,
// kick it off ourselves.
if (typeof YT !== "undefined" && YT && typeof YT.Player === "function") {
  window.onYouTubeIframeAPIReady();
}

function onPlayerReady() {
  clearPlayerInitTimeout();
  clearPlayerFallback();

  // Prefill partDurations from any cached real durations BEFORE any seek
  // logic runs. Without this, multi-part resumes compute global offsets
  // against the 3h MAX_PART_DURATION placeholder and chat lands on the
  // wrong content offset.
  var cachedDurations = getCachedPartDurations();
  if (cachedDurations) {
    for (var ci = 0; ci < cachedDurations.length; ci++) {
      if (cachedDurations[ci] > 0) {
        partDurations[ci] = cachedDurations[ci];
      }
    }
    // Real durations refine the player timeline, so recompute the chat delay
    // against the same array — otherwise getChatTime maps player time onto the
    // broadcast clock using stale server estimates and chat drifts.
    CHAT_DELAY = computeChatDelay(VOD_TOTAL_SECS, partDurations, MAX_PART_DURATION);
  }

  buildPartSelector();
  buildChapterSelector();

  // Resolve the initial position. Priority: ?t= deep-link > saved resume > start.
  var urlParams = new URLSearchParams(window.location.search);
  var deepLinkT = parseInt(urlParams.get("t"), 10);
  var resume = getResumePosition();
  var initialOffset = 0;
  if (deepLinkT > 0) {
    seekToGlobal(deepLinkT);
    initialOffset = deepLinkT;
  } else if (resume && resume.time > 10) {
    var hasLocal =
      typeof resume.localTime === "number" &&
      resume.part != null &&
      resume.part >= 0 &&
      resume.part < YOUTUBE_IDS.length;
    if (hasLocal) {
      // Precise resume: jump to the exact part + local offset, sidestepping
      // the partDurations placeholder entirely.
      if (resume.part !== currentPart) {
        switchPart(resume.part, resume.localTime);
        // switchPart already loaded chat with the correct offset, and we
        // still need the tick loop running. Skip the boot loadChat below.
        tickInterval = setInterval(tick, 1000);
        return;
      }
      if (resume.localTime > 0) {
        player.seekTo(resume.localTime, true);
      }
      // Same-part resume: this branch is only reached when resume.part ===
      // currentPart, and currentPart is always 0 here, so resume.part === 0
      // and the cumulative offset reduces to localTime.
      initialOffset = resume.localTime;
    } else {
      // Legacy entry (no localTime): fall back to global-time seek.
      seekToGlobal(resume.time);
      initialOffset = resume.time;
    }
  }

  // Load chat from the same offset the player will be at, NOT 0. Otherwise the
  // tick loop pages through every chat message from 0 → initialOffset trying to
  // catch up, which floods the DOM and visibly desyncs from the video.
  loadChat(initialOffset + CHAT_DELAY);

  // Start tick
  tickInterval = setInterval(tick, 1000);
}

function onPlayerStateChange(event) {
  if (event.data === YT.PlayerState.PLAYING) {
    try {
      var dur = player.getDuration();
      if (dur > 0) {
        partDurations[currentPart] = dur;
        savePartDuration(currentPart, dur);
      }
    } catch (e) {
      /* ignore */
    }
  }

  if (event.data === YT.PlayerState.ENDED) {
    // Auto-advance to next part
    if (currentPart < YOUTUBE_IDS.length - 1) {
      switchPart(currentPart + 1);
    } else {
      // All parts finished, clear resume and maybe offer next VOD in the same period
      finalizeCurrentVod();
      maybeShowUpNext();
    }
  }
}

function onPlayerError() {
  showPlayerFallback("api-failed");
}

// ─── Up Next (auto-continue to next VOD in playing period) ───

var UP_NEXT_SECONDS = 10;
var upNextInterval = null;
var upNextTriggered = false;
var pendingNextId = null;
var upNextOverlay = document.getElementById("up-next-overlay");
var upNextTitleEl = document.getElementById("up-next-title");
var upNextSecondsEl = document.getElementById("up-next-seconds");
var upNextCancelBtn = document.getElementById("up-next-cancel");
var upNextPlayNowBtn = document.getElementById("up-next-play-now");

if (upNextPlayNowBtn) {
  upNextPlayNowBtn.addEventListener("click", function () {
    if (!pendingNextId) return;
    var id = pendingNextId;
    cancelUpNext();
    navigateToNext(id);
  });
}
if (upNextCancelBtn) {
  upNextCancelBtn.addEventListener("click", cancelUpNext);
}

function cancelUpNext() {
  if (upNextInterval) {
    clearInterval(upNextInterval);
    upNextInterval = null;
  }
  if (upNextOverlay) upNextOverlay.hidden = true;
}

function navigateToNext(nextId) {
  var url = "/watch/" + encodeURIComponent(nextId);
  // Only propagate the game hint if the user explicitly arrived with one —
  // otherwise a single-chapter inference would lock them into a chain.
  if (HAS_EXPLICIT_HINT && GAME_HINT) {
    url += "?game=" + encodeURIComponent(GAME_HINT);
  }
  window.location.href = url;
}

function maybeShowUpNext() {
  if (!upNextOverlay) return;
  if (upNextTriggered) return;
  upNextTriggered = true;
  var url = "/api/next/" + encodeURIComponent(VOD_ID);
  if (GAME_HINT) url += "?game=" + encodeURIComponent(GAME_HINT);
  fetch(url)
    .then(function (res) {
      if (res.status === 204) return null;
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    })
    .then(function (data) {
      if (!data || !data.next_id) return;
      pendingNextId = data.next_id;
      upNextTitleEl.textContent = data.next_title || "Next stream";
      var remaining = UP_NEXT_SECONDS;
      upNextSecondsEl.textContent = String(remaining);
      upNextOverlay.hidden = false;
      upNextInterval = setInterval(function () {
        remaining -= 1;
        if (remaining <= 0) {
          var id = pendingNextId;
          cancelUpNext();
          navigateToNext(id);
          return;
        }
        upNextSecondsEl.textContent = String(remaining);
      }, 1000);
    })
    .catch(function (err) {
      console.warn("[UpNext] lookup failed:", err);
    });
}

// ─── Save on unload ───

window.addEventListener("beforeunload", function () {
  savePosition();
  cancelUpNext();
});

// ─── Theatre Mode ───

var THEATRE_KEY = "moonmoon_theatre";
var theatreBtn = document.getElementById("theatre-toggle");

function setTheatre(on) {
  document.body.classList.toggle("theatre-mode", on);
  theatreBtn.setAttribute("aria-pressed", on ? "true" : "false");
  theatreBtn.title = on ? "Exit theatre mode (t)" : "Theatre mode (t)";
  storageSet(storage, THEATRE_KEY, on ? "1" : "0");
}

// Restore saved preference
if (storageGet(storage, THEATRE_KEY) === "1") {
  setTheatre(true);
}

theatreBtn.addEventListener("click", function () {
  setTheatre(!document.body.classList.contains("theatre-mode"));
});

document.addEventListener("keydown", function (e) {
  if (
    e.target instanceof HTMLElement &&
    (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA")
  )
    return;
  if (e.key === "t" && !e.ctrlKey && !e.metaKey && !e.altKey) {
    setTheatre(!document.body.classList.contains("theatre-mode"));
  }
});
