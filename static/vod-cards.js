import { nextChapterPopoverOpen } from "./lib/chapter-popover.js";
import { hasWatchedVod } from "./lib/watched.js";

const RESUME_KEY = "moonmoon_resume";
const WATCHED_KEY = "moonmoon_watched";

function readStore(key) {
  try {
    return JSON.parse(localStorage.getItem(key)) || {};
  } catch (error) {
    return {};
  }
}

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

function applyResumeState(card, resumeStore) {
  const id = card.dataset.vodId;
  const duration = Number(card.dataset.durationSecs);
  const resume = resumeStore[id];
  const fill = card.querySelector(".resume-bar-fill");
  const historyState = card.dataset.historyState;
  const time =
    historyState === "in_progress"
      ? Number(card.dataset.progressSeconds)
      : Number(resume && resume.time);

  if (historyState === "watched") {
    card.classList.remove("has-resume");
    if (fill) fill.style.width = "0%";
    return;
  }

  if (
    Number.isFinite(time) &&
    time > 10 &&
    Number.isFinite(duration) &&
    duration > 0
  ) {
    const percent = Math.max(0, Math.min((time / duration) * 100, 100));
    card.classList.add("has-resume");
    if (fill) fill.style.width = `${percent}%`;
    return;
  }

  card.classList.remove("has-resume");
  if (fill) fill.style.width = "0%";
}

function applyWatchedState(card, watchedStore) {
  const historyState = card.dataset.historyState;
  const watched =
    historyState === "watched"
      ? true
      : historyState === "in_progress"
        ? false
        : hasWatchedVod(watchedStore, card.dataset.vodId);
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
  const resumeStore = readStore(RESUME_KEY);
  const watchedStore = readStore(WATCHED_KEY);
  const cards =
    root instanceof Element && root.matches(".vod-card[data-vod-id]")
      ? [root]
      : root.querySelectorAll(".vod-card[data-vod-id]");

  cards.forEach((card) => {
    applyResumeState(card, resumeStore);
    applyWatchedState(card, watchedStore);
  });
}

function applyFromEvent(event) {
  const root = (event.detail && event.detail.target) || document;
  applyVodCardState(root);
  initChapterPopovers(root);
}

applyVodCardState();
initChapterPopovers();

document.body.addEventListener("htmx:afterSwap", applyFromEvent);
window.addEventListener("moonmoon:resumeChanged", () => applyVodCardState());
window.addEventListener("moonmoon:watchedChanged", () => applyVodCardState());
window.addEventListener("storage", (event) => {
  if (event.key === RESUME_KEY || event.key === WATCHED_KEY)
    applyVodCardState();
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
    setChapterPopoverOpen(card, nextChapterPopoverOpen(true, { type: "escape" }));
  });
});
