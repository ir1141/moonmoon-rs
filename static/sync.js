import { mergeResume as mergeResumePure } from "./lib/resume.js";
import {
  isValidToken,
  generateToken as generateTokenPure,
} from "./lib/token.js";

var TOKEN_KEY = "moonmoon_sync_token";
var RESUME_KEY = "moonmoon_resume";
var META_KEY = "moonmoon_sync_meta";

function getToken() {
  var t = localStorage.getItem(TOKEN_KEY) || "";
  return isValidToken(t) ? t : "";
}

function setToken(t) {
  if (t && isValidToken(t)) {
    localStorage.setItem(TOKEN_KEY, t);
  } else {
    localStorage.removeItem(TOKEN_KEY);
  }
}

function generateToken() {
  return generateTokenPure(function (n) {
    var bytes = new Uint8Array(n);
    crypto.getRandomValues(bytes);
    return bytes;
  });
}

function getResume() {
  try {
    return JSON.parse(localStorage.getItem(RESUME_KEY)) || {};
  } catch (e) {
    return {};
  }
}

function setResume(obj) {
  try {
    localStorage.setItem(RESUME_KEY, JSON.stringify(obj));
  } catch (e) {
    console.warn("[Sync] resume write failed:", e);
  }
}

function mergeResume(remote) {
  var local = getResume();
  var result = mergeResumePure(local, remote);
  if (result.changed) setResume(result.merged);
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
      var remoteResume = (data.blob && data.blob.resume) || {};
      var changed = mergeResume(remoteResume);
      try {
        localStorage.setItem(
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
  var blob = { resume: getResume() };
  var body = JSON.stringify({ blob: blob, updated_at: Date.now() });
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

// localStorage `storage` events fire on OTHER tabs only, so we also poll
// the resume key in this tab. 2s is fine — the debounce already coalesces.
var lastResumeStr = localStorage.getItem(RESUME_KEY) || "";
setInterval(function () {
  var cur = localStorage.getItem(RESUME_KEY) || "";
  if (cur !== lastResumeStr) {
    lastResumeStr = cur;
    schedulePush();
  }
}, POLL_MS);

window.addEventListener("storage", function (e) {
  if (e.key === RESUME_KEY) {
    lastResumeStr = e.newValue || "";
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

function initSettingsUi() {
  var btn = el("sync-btn");
  var dlg = el("sync-dialog");
  if (!btn || !dlg) return;

  function refreshUi() {
    var token = getToken();
    var connected = !!token;
    btn.classList.toggle("connected", connected);
    btn.title = connected ? "Sync: connected" : "Cross-device sync";
    el("sync-status").textContent = connected
      ? "Connected. Your watch history syncs to any device using this token."
      : "Not connected. Generate a token and copy it to your other devices.";
    el("sync-token-block").hidden = !connected;
    el("sync-disconnect").hidden = !connected;
    if (connected) el("sync-token-value").value = token;
    el("sync-import-block").hidden = true;
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

  el("sync-generate").addEventListener("click", function () {
    var t = generateToken();
    setToken(t);
    refreshUi();
    // Push immediately so a fresh token is registered server-side.
    push();
  });

  el("sync-import-show").addEventListener("click", function () {
    el("sync-import-block").hidden = false;
    el("sync-import-input").focus();
  });

  el("sync-import-confirm").addEventListener("click", function () {
    var raw = (el("sync-import-input").value || "")
      .trim()
      .replace(/[\s-]/g, "")
      .toUpperCase();
    if (!isValidToken(raw)) {
      el("sync-status").textContent =
        "Invalid token. Expected 26+ characters of A–Z, 2–7.";
      return;
    }
    setToken(raw);
    refreshUi();
    pull().then(function () {
      el("sync-status").textContent = "Connected. Pulled remote history.";
    });
  });

  el("sync-disconnect").addEventListener("click", function () {
    setToken("");
    refreshUi();
  });

  el("sync-copy").addEventListener("click", function () {
    var v = el("sync-token-value").value;
    if (!v) return;
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(v);
    } else {
      el("sync-token-value").select();
      try {
        document.execCommand("copy");
      } catch (e) {
        /* ignore */
      }
    }
    var copyBtn = el("sync-copy");
    var orig = copyBtn.textContent;
    copyBtn.textContent = "Copied!";
    setTimeout(function () {
      copyBtn.textContent = orig;
    }, 1200);
  });

  refreshUi();
}

initSettingsUi();
