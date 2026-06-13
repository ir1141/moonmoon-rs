import { safeLocalStorage } from "./storage.js";

export const historySortKey = "moonmoon_history_sort";
export const defaultHistorySort = "recent";

const supportedHistorySorts = new Set([defaultHistorySort, "game"]);

/**
 * @typedef {{
 *   get?: (key: string) => unknown,
 *   getItem?: (key: string) => string | null,
 *   set?: (key: string, value: string) => unknown,
 *   setItem?: (key: string, value: string) => unknown,
 * }} HistorySortStorage
 */

/**
 * @param {unknown} value
 */
export function normalizeHistorySort(value) {
  return typeof value === "string" && supportedHistorySorts.has(value)
    ? value
    : defaultHistorySort;
}

/**
 * @param {HistorySortStorage} [storage]
 */
export function readHistorySort(storage = safeLocalStorage()) {
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

/**
 * @param {HistorySortStorage} storage
 * @param {unknown} value
 */
export function writeHistorySort(storage, value) {
  storage = storage || safeLocalStorage();
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
