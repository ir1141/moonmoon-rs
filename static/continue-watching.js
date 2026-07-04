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

async function initContinueWatching() {
  const shelf = document.getElementById("continue-watching");
  const hero = document.getElementById("continue-hero");
  if (!shelf || !hero) return;

  const [entry] = selectContinueWatchingEntries(
    loadHistoryStore(safeLocalStorage()),
  );
  if (!entry) return;

  try {
    const response = await fetch(buildContinueResumeUrl(entry));
    if (!response.ok) return;

    const html = (await response.text()).trim();
    if (!html) return;

    const template = document.createElement("template");
    template.innerHTML = html;
    hero.replaceChildren(template.content);
    fillProgressBar(hero);
    shelf.hidden = false;
  } catch (error) {
    console.warn("[ContinueWatching] failed to load:", error);
  }
}

initContinueWatching();
