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

function listRegionFromEvent(event) {
  const detailTarget = event.detail && event.detail.target;
  const target =
    detailTarget instanceof Element
      ? detailTarget
      : event.target instanceof Element
        ? event.target
        : null;

  if (!target) return null;
  return target.matches("[data-list-region]")
    ? target
    : target.closest("[data-list-region]");
}

function setListBusyFromEvent(event) {
  const region = listRegionFromEvent(event);
  if (region) region.setAttribute("aria-busy", "true");
}

function clearListBusy() {
  document
    .querySelectorAll('[data-list-region][aria-busy="true"]')
    .forEach((region) => {
      region.setAttribute("aria-busy", "false");
    });
}

function parameterIsEmpty(value) {
  if (Array.isArray(value)) return value.every(parameterIsEmpty);
  return value == null || String(value).trim() === "";
}

function deleteParameter(parameters, key) {
  if (parameters instanceof FormData) {
    const values = parameters.getAll(key);
    if (!values.length || values.every(parameterIsEmpty)) parameters.delete(key);
    return;
  }

  if (
    parameters &&
    Object.prototype.hasOwnProperty.call(parameters, key) &&
    parameterIsEmpty(parameters[key])
  ) {
    delete parameters[key];
  }
}

function pruneEmptyListParameters(event) {
  const detail = event.detail || {};
  const elt = detail.elt instanceof Element ? detail.elt : null;
  const form = elt && (elt.matches("#vod-filters") ? elt : elt.closest("#vod-filters"));
  if (!form) return;

  ["search", "from", "to", "page"].forEach((key) => {
    deleteParameter(detail.parameters, key);
  });
}

applyVodCardState();

document.body.addEventListener("htmx:configRequest", pruneEmptyListParameters);
document.body.addEventListener("htmx:beforeRequest", setListBusyFromEvent);
document.body.addEventListener("htmx:afterSettle", clearListBusy);
document.body.addEventListener("htmx:responseError", clearListBusy);
document.body.addEventListener("htmx:sendError", clearListBusy);
document.body.addEventListener("htmx:afterSwap", applyFromEvent);
window.addEventListener("moonmoon:resumeChanged", () => applyVodCardState());
window.addEventListener("moonmoon:watchedChanged", () => applyVodCardState());
window.addEventListener("storage", (event) => {
  if (event.key === RESUME_KEY || event.key === WATCHED_KEY)
    applyVodCardState();
});
