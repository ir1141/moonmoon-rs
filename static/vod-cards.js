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
  const time = Number(resume && resume.time);

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
  const watched = hasWatchedVod(watchedStore, card.dataset.vodId);
  const existingBadge = card.querySelector(".watched-badge");

  card.classList.toggle("watched", watched);

  if (watched && !existingBadge) {
    const thumbWrap = card.querySelector(".thumb-wrap");
    if (thumbWrap) thumbWrap.appendChild(buildWatchedBadge());
  } else if (!watched && existingBadge) {
    existingBadge.remove();
  }
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
  applyVodCardState((event.detail && event.detail.target) || document);
}

applyVodCardState();

document.body.addEventListener("htmx:afterSwap", applyFromEvent);
window.addEventListener("moonmoon:resumeChanged", () => applyVodCardState());
window.addEventListener("moonmoon:watchedChanged", () => applyVodCardState());
window.addEventListener("storage", (event) => {
  if (event.key === RESUME_KEY || event.key === WATCHED_KEY)
    applyVodCardState();
});
