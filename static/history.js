import {
  buildHistoryEntries,
  readJsonStore,
  serializeHistoryRequest,
} from "./lib/history-state.js";
import { readHistorySort, writeHistorySort } from "./lib/history-sort.js";

const RESUME_KEY = "moonmoon_resume";
const WATCHED_KEY = "moonmoon_watched";

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

  const entries = buildHistoryEntries(
    readJsonStore(localStorage, RESUME_KEY),
    readJsonStore(localStorage, WATCHED_KEY),
  );

  if (entries.length === 0) {
    stats.textContent = "No watch history";
    showMessage(grid, "No watch history");
    return;
  }

  stats.textContent =
    entries.length === 1 ? "1 history entry" : `${entries.length} history entries`;

  function load() {
    const params = serializeHistoryRequest(entries, sortSel.value);

    fetch(`/history/vods?${params.toString()}`)
      .then((response) => {
        if (!response.ok) throw new Error("history fetch failed");
        return response.text();
      })
      .then((html) => {
        grid.replaceChildren();
        const temp = document.createElement("template");
        temp.innerHTML = html;
        grid.appendChild(temp.content);
        const event = new CustomEvent("htmx:afterSwap", {
          detail: { target: grid },
        });
        document.body.dispatchEvent(event);
        if (!grid.querySelector(".vod-card")) {
          showMessage(grid, "No matching archived streams found");
        }
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
