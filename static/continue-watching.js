import {
  buildContinueResumeUrl,
  selectContinueWatchingEntries,
} from "./lib/continue-watching.js";

const RESUME_KEY = "moonmoon_resume";

function readResumeStore() {
  try {
    return JSON.parse(localStorage.getItem(RESUME_KEY)) || {};
  } catch (error) {
    return {};
  }
}

async function initContinueWatching() {
  const shelf = document.getElementById("continue-watching");
  const hero = document.getElementById("continue-hero");
  if (!shelf || !hero) return;

  const [entry] = selectContinueWatchingEntries(readResumeStore());
  if (!entry) return;

  try {
    const response = await fetch(buildContinueResumeUrl(entry));
    if (!response.ok) return;

    const html = (await response.text()).trim();
    if (!html) return;

    const template = document.createElement("template");
    template.innerHTML = html;
    hero.replaceChildren(template.content);
    shelf.hidden = false;
  } catch (error) {
    console.warn("[ContinueWatching] failed to load:", error);
  }
}

initContinueWatching();
