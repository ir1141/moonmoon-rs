import {
  buildHistoryEntries,
  buildHistoryRequest,
  loadHistoryStore,
} from "./lib/history-state.js";
import { readHistorySort, writeHistorySort } from "./lib/history-sort.js";
import { safeLocalStorage } from "./lib/storage.js";
import { initVodCards } from "./vod-cards.js";

const storage = safeLocalStorage();

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
  const sortInput = /** @type {HTMLInputElement | null} */ (
    document.getElementById("history-sort")
  );
  if (!stats || !grid || !sortInput) return;

  function applySortToControl(value) {
    sortInput.value = value;
    const control = sortInput.closest("[data-sort-control]");
    const item =
      control && control.querySelector(`.sort-item[data-value="${value}"]`);
    const label = control && control.querySelector("[data-sort-label]");
    if (item instanceof HTMLElement && label instanceof HTMLElement) {
      label.innerHTML = `<b>Sort:</b> ${item.dataset.label}`;
      control.querySelectorAll(".sort-item").forEach((opt) => {
        const active = opt === item;
        opt.classList.toggle("is-active", active);
        opt.setAttribute("aria-selected", active ? "true" : "false");
      });
    }
  }

  applySortToControl(readHistorySort());

  const entries = buildHistoryEntries(loadHistoryStore(storage));

  if (entries.length === 0) {
    stats.textContent = "No watch history";
    showMessage(grid, "No watch history");
    return;
  }

  stats.textContent =
    entries.length === 1
      ? "1 history entry"
      : `${entries.length} history entries`;

  let loadGeneration = 0;

  function load() {
    const generation = ++loadGeneration;

    fetch("/history/vods", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(buildHistoryRequest(entries, sortInput.value)),
    })
      .then((response) => {
        if (!response.ok)
          throw new Error(`history fetch failed: HTTP ${response.status}`);
        return response.text();
      })
      .then((html) => {
        if (generation !== loadGeneration) return; // a newer load superseded this one
        grid.replaceChildren();
        const temp = document.createElement("template");
        temp.innerHTML = html;
        grid.appendChild(temp.content);
        initVodCards(grid);
        if (!grid.querySelector(".vod-card")) {
          showMessage(grid, "No matching archived streams found");
        }
      })
      .catch((err) => {
        if (generation !== loadGeneration) return;
        console.warn("[History] load failed:", err);
        showMessage(grid, "Failed to load history");
      });
  }

  sortInput.addEventListener("change", () => {
    writeHistorySort(storage, sortInput.value);
    load();
  });
  load();
}

initHistoryPage();
