import {
  buildContinueResumeUrl,
  selectContinueWatchingEntries,
} from "./lib/continue-watching.js";
import { loadHistoryStore, resumePercent } from "./lib/history-state.js";
import { safeLocalStorage } from "./lib/storage.js";

function fillProgressBar(root) {
  const bar = root.querySelector(".continue-progress");
  const fill = bar && bar.querySelector("span");
  if (!(bar instanceof HTMLElement) || !fill) return;
  fill.style.width = `${resumePercent(
    bar.dataset.resumeSeconds,
    bar.dataset.durationSecs,
  )}%`;
}

/**
 * Collapse the skeleton reserved before first paint by the inline script in
 * continue_watching_block.html. Called on every path that yields no card, so a
 * failed fetch or a stale reservation can't strand a placeholder on the page.
 */
function clearReservation() {
  document.documentElement.removeAttribute("data-continue");
}

async function initContinueWatching() {
  const shelf = document.getElementById("continue-watching");
  const hero = document.getElementById("continue-hero");
  if (!shelf || !hero) return;

  const [entry] = selectContinueWatchingEntries(
    loadHistoryStore(safeLocalStorage()),
  );
  if (!entry) {
    clearReservation();
    return;
  }

  try {
    const response = await fetch(buildContinueResumeUrl(entry));
    if (!response.ok) {
      clearReservation();
      return;
    }

    // A 204 is `ok` but carries no card (the VOD left the catalog).
    const html = (await response.text()).trim();
    if (!html) {
      clearReservation();
      return;
    }

    const template = document.createElement("template");
    template.innerHTML = html;
    hero.replaceChildren(template.content);
    fillProgressBar(hero);
    hero.setAttribute("aria-busy", "false");
  } catch (error) {
    clearReservation();
    console.warn("[ContinueWatching] failed to load:", error);
  }
}

initContinueWatching();
