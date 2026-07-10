import {
  isValidToken,
  generateToken as generateTokenPure,
} from "./lib/token.js";
import { safeLocalStorage } from "./lib/storage.js";
import { createSyncSession } from "./lib/sync-session.js";

// localStorage access throws SecurityError in storage-blocking browsers; a
// bare module-eval call would abort the whole sync module, so all access
// goes through the lib/storage.js guards against this handle.
var storage = safeLocalStorage();

function generateToken() {
  return generateTokenPure(function (n) {
    var bytes = new Uint8Array(n);
    crypto.getRandomValues(bytes);
    return bytes;
  });
}

var transport = {
  pull: function (token) {
    return fetch("/api/sync/" + encodeURIComponent(token)).then(function (res) {
      if (res.status === 404) return null;
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    });
  },
  push: function (token, body) {
    return fetch("/api/sync/" + encodeURIComponent(token), {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      keepalive: true,
    }).then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
    });
  },
};

var runtime = {
  now: function () {
    return Date.now();
  },
  isVisible: function () {
    return !document.visibilityState || document.visibilityState !== "hidden";
  },
  onWindow: function (type, listener) {
    window.addEventListener(type, listener);
    return function () {
      if (window.removeEventListener)
        window.removeEventListener(type, listener);
    };
  },
  onDocument: function (type, listener) {
    if (!document.addEventListener) return function () {};
    document.addEventListener(type, listener);
    return function () {
      if (document.removeEventListener)
        document.removeEventListener(type, listener);
    };
  },
  dispatchHistoryChanged: function () {
    window.dispatchEvent(new Event("moonmoon:historyChanged"));
  },
  setTimeout: function (listener, delay) {
    return setTimeout(listener, delay);
  },
  clearTimeout: function (id) {
    clearTimeout(id);
  },
  setInterval: function (listener, delay) {
    return setInterval(listener, delay);
  },
  clearInterval: function (id) {
    clearInterval(id);
  },
  warn: function (message, error) {
    console.warn("[Sync] " + message + ":", error);
  },
};

var session = createSyncSession({
  storage: storage,
  transport: transport,
  runtime: runtime,
});
var getToken = session.getToken;
var setToken = session.setToken;
var pull = session.pull;
var push = session.push;

window.__moonmoonSync = {
  getToken: getToken,
  setToken: setToken,
  isValidToken: isValidToken,
  generateToken: generateToken,
  connect: session.connect,
  disconnect: session.disconnect,
  pull: pull,
  push: push,
};

void session.start();

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
    var connection = session.connect(raw);
    refreshUi();
    connection.then(function () {
      status.textContent = "Connected. Pulled remote history.";
    });
  });

  disconnectBtn.addEventListener("click", function () {
    session.disconnect();
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
