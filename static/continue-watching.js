import {
  buildContinueWatchingUrl,
  resumePercent,
  selectContinueWatchingEntries,
} from "./lib/continue-watching.js";

const RESUME_KEY = "moonmoon_resume";
const COLLAPSED_KEY = "moonmoon_continue_collapsed";
const LIMIT = 4;

function readResumeStore() {
  try {
    return JSON.parse(localStorage.getItem(RESUME_KEY)) || {};
  } catch (error) {
    return {};
  }
}

function applyProgressByOrder(grid, entries) {
  const cards = grid.querySelectorAll(".vod-card[data-vod-id]");

  cards.forEach((card, index) => {
    const entry = entries[index];
    if (!entry) return;

    const percent = resumePercent(entry.time, card.dataset.durationSecs);
    if (percent <= 0) return;

    card.classList.add("has-resume");
    const fill = card.querySelector(".resume-bar-fill");
    if (fill) fill.style.width = `${percent}%`;
  });
}

function isCollapsed() {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === "true";
  } catch (error) {
    return false;
  }
}

function setCollapsed(collapsed) {
  try {
    localStorage.setItem(COLLAPSED_KEY, collapsed ? "true" : "false");
  } catch (error) {
    // Ignore storage failures; the button should still work for this page load.
  }
}

function applyCollapsedState(shelf, toggle, collapsed) {
  shelf.classList.toggle("is-collapsed", collapsed);
  toggle.setAttribute("aria-expanded", collapsed ? "false" : "true");
  toggle.setAttribute(
    "aria-label",
    collapsed ? "Expand continue watching" : "Collapse continue watching",
  );
  toggle.setAttribute("title", collapsed ? "Expand" : "Collapse");
}

function initMinimizeToggle(shelf) {
  const toggle = document.getElementById("continue-toggle");
  if (!toggle) return;

  applyCollapsedState(shelf, toggle, isCollapsed());
  toggle.addEventListener("click", () => {
    const next = !shelf.classList.contains("is-collapsed");
    applyCollapsedState(shelf, toggle, next);
    setCollapsed(next);
  });
}

async function initContinueWatching() {
  const shelf = document.getElementById("continue-watching");
  const grid = document.getElementById("continue-grid");
  if (!shelf || !grid) return;

  const entries = selectContinueWatchingEntries(readResumeStore(), { limit: LIMIT });
  if (entries.length === 0) return;

  try {
    const response = await fetch(buildContinueWatchingUrl(entries));
    if (!response.ok) return;

    const html = await response.text();
    const template = document.createElement("template");
    template.innerHTML = html;

    if (!template.content.querySelector(".vod-card")) return;

    grid.replaceChildren(template.content);
    applyProgressByOrder(grid, entries);
    initMinimizeToggle(shelf);
    shelf.hidden = false;

    document.body.dispatchEvent(
      new CustomEvent("htmx:afterSwap", { detail: { target: grid } }),
    );
  } catch (error) {
    console.warn("[ContinueWatching] failed to load:", error);
  }
}

initContinueWatching();
