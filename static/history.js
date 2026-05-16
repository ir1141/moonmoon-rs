import { readHistorySort, writeHistorySort } from "./lib/history-sort.js";

const RESUME_KEY = "moonmoon_resume";

function readResumeStore() {
  try {
    return JSON.parse(localStorage.getItem(RESUME_KEY)) || {};
  } catch (error) {
    return {};
  }
}

function buildHistoryEntries(store) {
  const entries = [];

  for (const id in store) {
    if (store[id] && store[id].updated) {
      entries.push({
        id,
        updated: store[id].updated,
        time: Math.floor(store[id].time || 0),
      });
    }
  }

  return entries.sort((a, b) => b.updated - a.updated);
}

function showMessage(grid, text) {
  grid.replaceChildren();
  const msg = document.createElement("div");
  msg.className = "no-results";
  msg.textContent = text;
  grid.appendChild(msg);
}

function initHistoryPage() {
  const stats = document.getElementById("history-stats");
  const grid = document.getElementById("history-grid");
  const sortSel = /** @type {HTMLSelectElement | null} */ (
    document.getElementById("history-sort")
  );
  if (!stats || !grid || !sortSel) return;

  sortSel.value = readHistorySort();

  const entries = buildHistoryEntries(readResumeStore());

  if (entries.length === 0) {
    stats.textContent = "No watch history";
    showMessage(grid, "No streams watched yet");
    return;
  }

  stats.textContent = `${entries.length} watched`;

  const ids = entries.map((entry) => entry.id).join(",");
  const times = entries.map((entry) => entry.time).join(",");

  function load() {
    const params = new URLSearchParams({
      ids,
      times,
      sort: sortSel.value,
    });

    fetch(`/history/vods?${params.toString()}`)
      .then((response) => response.text())
      .then((html) => {
        grid.replaceChildren();
        const temp = document.createElement("template");
        temp.innerHTML = html;
        grid.appendChild(temp.content);
        const event = new CustomEvent("htmx:afterSwap", {
          detail: { target: grid },
        });
        document.body.dispatchEvent(event);
      })
      .catch(() => {
        showMessage(grid, "Failed to load history");
      });
  }

  sortSel.addEventListener("change", () => {
    sortSel.value = writeHistorySort(localStorage, sortSel.value);
    load();
  });
  load();
}

initHistoryPage();
