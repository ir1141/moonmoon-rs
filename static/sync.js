import {
  HISTORY_KEY,
  buildSyncBlob,
  historyFromBlob,
  loadHistoryStore,
  mergeHistory,
  saveHistoryStore,
} from "./lib/history-state.js";
import {
  isValidToken,
  generateToken as generateTokenPure,
} from "./lib/token.js";
import {
  safeLocalStorage,
  storageGet,
  storageRemove,
  storageSet,
} from "./lib/storage.js";

var TOKEN_KEY = "moonmoon_sync_token";
var META_KEY = "moonmoon_sync_meta";

// localStorage access throws SecurityError in storage-blocking browsers; a
// bare module-eval call would abort the whole sync module, so all access
// goes through the lib/storage.js guards against this handle.
var storage = safeLocalStorage();

function getToken() {
  var t = storageGet(storage, TOKEN_KEY) || "";
  return isValidToken(t) ? t : "";
}

function setToken(t) {
  if (t && isValidToken(t)) {
    storageSet(storage, TOKEN_KEY, t);
  } else {
    storageRemove(storage, TOKEN_KEY);
  }
}

function generateToken() {
  return generateTokenPure(function (n) {
    var bytes = new Uint8Array(n);
    crypto.getRandomValues(bytes);
    return bytes;
  });
}

function getHistory() {
  return loadHistoryStore(storage);
}

function setHistory(store) {
  try {
    saveHistoryStore(storage, store);
    window.dispatchEvent(new Event("moonmoon:historyChanged"));
  } catch (e) {
    console.warn("[Sync] history write failed:", e);
  }
}

function mergeRemoteHistory(remote) {
  var result = mergeHistory(getHistory(), remote);
  if (result.changed) setHistory(result.merged);
  return result.changed;
}

function pull() {
  var token = getToken();
  if (!token) return Promise.resolve(null);
  return fetch("/api/sync/" + encodeURIComponent(token))
    .then(function (res) {
      if (res.status === 404) return null;
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    })
    .then(function (data) {
      if (!data) return null;
      var changed = mergeRemoteHistory(historyFromBlob(data.blob));
      try {
        storageSet(
          storage,
          META_KEY,
          JSON.stringify({
            last_pulled_updated_at: data.updated_at || 0,
            last_pulled_at: Date.now(),
          }),
        );
      } catch (e) {
        /* ignore */
      }
      return { changed: changed, updated_at: data.updated_at };
    })
    .catch(function (err) {
      console.warn("[Sync] pull failed:", err);
      return null;
    });
}

var DEBOUNCE_MS = 3000;
var POLL_MS = 2000;
var pushTimer = null;

function push() {
  pushTimer = null;
  var token = getToken();
  if (!token) return;
  var body = JSON.stringify({
    blob: buildSyncBlob(getHistory()),
    updated_at: Date.now(),
  });
  fetch("/api/sync/" + encodeURIComponent(token), {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: body,
    keepalive: true,
  }).catch(function (err) {
    console.warn("[Sync] push failed:", err);
  });
}

function schedulePush() {
  if (!getToken()) return;
  if (pushTimer) clearTimeout(pushTimer);
  pushTimer = setTimeout(push, DEBOUNCE_MS);
}

// Run the legacy-store migration eagerly — sync.js loads on every page, so
// this is the one guaranteed migration point even before any history read.
loadHistoryStore(storage);

// localStorage `storage` events fire on OTHER tabs only, so we also poll
// the history key in this tab. 2s is fine — the debounce already coalesces.
var lastHistoryStr = storageGet(storage, HISTORY_KEY) || "";
setInterval(function () {
  var cur = storageGet(storage, HISTORY_KEY) || "";
  if (cur !== lastHistoryStr) {
    lastHistoryStr = cur;
    schedulePush();
  }
}, POLL_MS);

window.addEventListener("storage", function (e) {
  if (e.key === HISTORY_KEY) {
    lastHistoryStr = storageGet(storage, HISTORY_KEY) || "";
    schedulePush();
  }
});

// Flush any pending push when the user navigates away. `keepalive: true`
// on the fetch lets it survive page unload.
window.addEventListener("beforeunload", function () {
  if (pushTimer) {
    clearTimeout(pushTimer);
    push();
  }
});

window.__moonmoonSync = {
  getToken: getToken,
  setToken: setToken,
  isValidToken: isValidToken,
  generateToken: generateToken,
  pull: pull,
  push: push,
};

pull();

// ─── Settings UI ───
function el(id) {
  return document.getElementById(id);
}

function requiredEl(id) {
  var node = el(id);
  if (!node) throw new Error("[Sync] Missing #" + id + " element");
  return node;
}

function requiredInput(id) {
  var node = requiredEl(id);
  if (!(node instanceof HTMLInputElement)) {
    throw new Error("[Sync] Expected #" + id + " to be an input");
  }
  return node;
}

function initSettingsUi() {
  var btn = el("sync-btn");
  var dlg = /** @type {HTMLDialogElement | null} */ (el("sync-dialog"));
  if (!btn || !dlg) return;
  var status = requiredEl("sync-status");
  var tokenBlock = requiredEl("sync-token-block");
  var disconnectBtn = requiredEl("sync-disconnect");
  var tokenValue = requiredInput("sync-token-value");
  var importBlock = requiredEl("sync-import-block");
  var generateBtn = requiredEl("sync-generate");
  var importShowBtn = requiredEl("sync-import-show");
  var importInput = requiredInput("sync-import-input");
  var importConfirmBtn = requiredEl("sync-import-confirm");
  var copyBtn = requiredEl("sync-copy");

  function refreshUi() {
    var token = getToken();
    var connected = !!token;
    btn.classList.toggle("connected", connected);
    btn.title = connected ? "Sync: connected" : "Cross-device sync";
    status.textContent = connected
      ? "Connected. Your watch history syncs to any device using this token."
      : "Not connected. Generate a token and copy it to your other devices.";
    tokenBlock.hidden = !connected;
    disconnectBtn.hidden = !connected;
    if (connected) tokenValue.value = token;
    importBlock.hidden = true;
  }

  function positionDialog() {
    var btnRect = btn.getBoundingClientRect();
    var dlgRect = dlg.getBoundingClientRect();
    var gap = 8;
    var pad = 8;
    var left = btnRect.right - dlgRect.width;
    var top = btnRect.bottom + gap;
    if (left < pad) left = pad;
    var maxLeft = window.innerWidth - dlgRect.width - pad;
    if (left > maxLeft) left = maxLeft;
    if (top + dlgRect.height > window.innerHeight - pad) {
      top = Math.max(pad, window.innerHeight - dlgRect.height - pad);
    }
    dlg.style.top = top + "px";
    dlg.style.left = left + "px";
  }

  btn.addEventListener("click", function () {
    refreshUi();
    if (typeof dlg.showModal === "function") dlg.showModal();
    else dlg.setAttribute("open", "");
    positionDialog();
  });

  window.addEventListener("resize", function () {
    if (dlg.open) positionDialog();
  });

  dlg.addEventListener("click", function (e) {
    if (e.target === dlg) dlg.close();
  });

  generateBtn.addEventListener("click", function () {
    var t = generateToken();
    setToken(t);
    refreshUi();
    // Push immediately so a fresh token is registered server-side.
    push();
  });

  importShowBtn.addEventListener("click", function () {
    importBlock.hidden = false;
    importInput.focus();
  });

  importConfirmBtn.addEventListener("click", function () {
    var raw = (importInput.value || "")
      .trim()
      .replace(/[\s-]/g, "")
      .toUpperCase();
    if (!isValidToken(raw)) {
      status.textContent =
        "Invalid token. Expected 26+ characters of A–Z, 2–7.";
      return;
    }
    setToken(raw);
    refreshUi();
    pull().then(function () {
      status.textContent = "Connected. Pulled remote history.";
    });
  });

  disconnectBtn.addEventListener("click", function () {
    setToken("");
    refreshUi();
  });

  copyBtn.addEventListener("click", function () {
    var v = tokenValue.value;
    if (!v) return;
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(v);
    } else {
      tokenValue.select();
      try {
        document.execCommand("copy");
      } catch (e) {
        /* ignore */
      }
    }
    var orig = copyBtn.textContent;
    copyBtn.textContent = "Copied!";
    setTimeout(function () {
      copyBtn.textContent = orig;
    }, 1200);
  });

  refreshUi();
}

initSettingsUi();
