import { nextChapterPopoverOpen } from "./lib/chapter-popover.js";
import {
  HISTORY_KEY,
  RESUME_MIN_SECONDS,
  loadHistoryStore,
  resumePercent,
} from "./lib/history-state.js";
import { safeLocalStorage } from "./lib/storage.js";

// Resolved through a guard: bare `localStorage` access throws in
// storage-blocking browsers, which would abort this module at eval — and
// history.js imports us, so it would take the history page down too.
const storage = safeLocalStorage();

function buildWatchedBadge() {
  const badge = document.createElement("div");
  badge.className = "watched-badge";
  badge.setAttribute("aria-label", "Watched");
  badge.setAttribute("title", "Watched");

  badge.innerHTML =
    '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">' +
    '<path d="M20 6 9 17l-5-5"></path>' +
    "</svg>";

  return badge;
}

// Server-rendered history pages stamp data-history-state/-progress-seconds on
// the card; those win. Cards without them (browse, home) are decorated from
// the local store.
function applyResumeState(card, store) {
  const id = card.dataset.vodId;
  const duration = Number(card.dataset.durationSecs);
  const entry = store[id];
  const fill = card.querySelector(".resume-bar-fill");
  const historyState = card.dataset.historyState;
  const time =
    historyState === "in_progress"
      ? Number(card.dataset.progressSeconds)
      : Number(entry && entry.state === "in_progress" && entry.time);

  if (historyState === "watched") {
    card.classList.remove("has-resume");
    if (fill) fill.style.width = "0%";
    return;
  }

  if (
    Number.isFinite(time) &&
    time > RESUME_MIN_SECONDS &&
    Number.isFinite(duration) &&
    duration > 0
  ) {
    const percent = resumePercent(time, duration);
    card.classList.add("has-resume");
    if (fill) fill.style.width = `${percent}%`;
    return;
  }

  card.classList.remove("has-resume");
  if (fill) fill.style.width = "0%";
}

function applyWatchedState(card, store) {
  const historyState = card.dataset.historyState;
  const entry = store[card.dataset.vodId];
  const watched =
    historyState === "watched"
      ? true
      : historyState === "in_progress"
        ? false
        : !!(entry && entry.state === "watched");
  const existingBadge = card.querySelector(".watched-badge");

  card.classList.toggle("watched", watched);

  if (watched && !existingBadge) {
    const thumbWrap = card.querySelector(".thumb-wrap");
    if (thumbWrap) thumbWrap.appendChild(buildWatchedBadge());
  } else if (!watched && existingBadge) {
    existingBadge.remove();
  }
}

function setChapterPopoverOpen(card, open) {
  const chip = card.querySelector(".game-count-chip");
  const popover = card.querySelector(".chapter-pop");
  if (!chip || !popover) return;

  chip.setAttribute("aria-expanded", open ? "true" : "false");
  popover.hidden = !open;
  card.classList.toggle("chapter-pop-open", open);
}

function closeChapterPopovers(exceptCard = null) {
  document.querySelectorAll(".vod-card.chapter-pop-open").forEach((card) => {
    if (card !== exceptCard) setChapterPopoverOpen(card, false);
  });
}

/**
 * @param {Document | Element} [root]
 */
function initChapterPopovers(root = document) {
  const cards =
    root instanceof Element && root.matches(".vod-card")
      ? [root]
      : root.querySelectorAll(".vod-card");

  cards.forEach((card) => {
    if (card.dataset.chapterPopoverReady === "true") return;

    const chip = card.querySelector(".game-count-chip");
    const popover = card.querySelector(".chapter-pop");
    if (!chip || !popover) return;

    card.dataset.chapterPopoverReady = "true";
    chip.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const nextOpen = nextChapterPopoverOpen(
        card.classList.contains("chapter-pop-open"),
        { type: "chip" },
      );
      closeChapterPopovers(card);
      setChapterPopoverOpen(card, nextOpen);
    });

    popover.addEventListener("click", (event) => {
      if (event.target instanceof Element && event.target.closest("a")) {
        setChapterPopoverOpen(card, false);
      }
    });
  });
}

/**
 * @param {Document | Element} [root]
 */
export function applyVodCardState(root = document) {
  const store = loadHistoryStore(storage);
  const cards =
    root instanceof Element && root.matches(".vod-card[data-vod-id]")
      ? [root]
      : root.querySelectorAll(".vod-card[data-vod-id]");

  cards.forEach((card) => {
    applyResumeState(card, store);
    applyWatchedState(card, store);
  });
}

/**
 * Re-apply card state and wire popovers for freshly inserted cards. Pages that
 * swap grid HTML manually (without htmx) call this directly.
 * @param {Document | Element} [root]
 */
export function initVodCards(root = document) {
  applyVodCardState(root);
  initChapterPopovers(root);
}

function applyFromEvent(event) {
  initVodCards((event.detail && event.detail.target) || document);
}

applyVodCardState();
initChapterPopovers();

document.body.addEventListener("htmx:afterSwap", applyFromEvent);
window.addEventListener("moonmoon:historyChanged", () => applyVodCardState());
window.addEventListener("storage", (event) => {
  if (event.key === HISTORY_KEY) applyVodCardState();
});

document.addEventListener("click", (event) => {
  const target = event.target;
  document.querySelectorAll(".vod-card.chapter-pop-open").forEach((card) => {
    if (target instanceof Node && card.contains(target)) return;
    setChapterPopoverOpen(
      card,
      nextChapterPopoverOpen(true, { type: "outside" }),
    );
  });
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  document.querySelectorAll(".vod-card.chapter-pop-open").forEach((card) => {
    setChapterPopoverOpen(
      card,
      nextChapterPopoverOpen(true, { type: "escape" }),
    );
  });
});

// Hide broken game-art images. We can't use an inline `onerror` attribute: the
// page CSP has no 'unsafe-inline'/'unsafe-hashes' in script-src, so the browser
// blocks inline handlers. `error` events don't bubble, so listen in the capture
// phase at the document root — this also covers cards htmx swaps in later
// without any per-card re-binding.
document.addEventListener(
  "error",
  (event) => {
    const el = event.target;
    if (el instanceof HTMLImageElement && el.classList.contains("art")) {
      el.style.display = "none";
    }
  },
  true,
);
