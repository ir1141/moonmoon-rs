// localStorage can throw in storage-blocking browsers — both the
// `window.localStorage` property access itself (SecurityError) and individual
// getItem/setItem calls (SecurityError, QuotaExceededError). Resolve the
// handle through safeLocalStorage() and route every access through these
// guards; a null storage degrades to no-op reads and writes.

export function safeLocalStorage() {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch (e) {
    return null;
  }
}

export function storageGet(storage, key) {
  if (!storage) return null;
  try {
    return storage.getItem(key);
  } catch (e) {
    return null;
  }
}

export function storageSet(storage, key, value) {
  if (!storage) return;
  try {
    storage.setItem(key, value);
  } catch (e) {
    /* storage blocked or quota exceeded */
  }
}

export function storageRemove(storage, key) {
  if (!storage) return;
  try {
    storage.removeItem(key);
  } catch (e) {
    /* storage blocked */
  }
}
