export const historySortKey = "moonmoon_history_sort";
export const defaultHistorySort = "recent";

const supportedHistorySorts = new Set([defaultHistorySort, "game"]);

export function normalizeHistorySort(value) {
  return supportedHistorySorts.has(value) ? value : defaultHistorySort;
}

export function readHistorySort(storage = localStorage) {
  try {
    if (storage && typeof storage.getItem === "function") {
      return normalizeHistorySort(storage.getItem(historySortKey));
    }
    if (storage && typeof storage.get === "function") {
      return normalizeHistorySort(storage.get(historySortKey));
    }
  } catch (error) {
    // Ignore storage failures; the default sort still works.
  }

  return defaultHistorySort;
}

export function writeHistorySort(storage = localStorage, value) {
  const sort = normalizeHistorySort(value);

  try {
    if (storage && typeof storage.setItem === "function") {
      storage.setItem(historySortKey, sort);
    } else if (storage && typeof storage.set === "function") {
      storage.set(historySortKey, sort);
    }
  } catch (error) {
    // Ignore storage failures; the in-page selection still works.
  }

  return sort;
}
