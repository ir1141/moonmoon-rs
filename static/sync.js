(function () {
  'use strict';

  var TOKEN_KEY = 'moonmoon_sync_token';
  var RESUME_KEY = 'moonmoon_resume';
  var META_KEY = 'moonmoon_sync_meta';

  function getToken() {
    var t = localStorage.getItem(TOKEN_KEY) || '';
    return isValidToken(t) ? t : '';
  }

  function setToken(t) {
    if (t && isValidToken(t)) {
      localStorage.setItem(TOKEN_KEY, t);
    } else {
      localStorage.removeItem(TOKEN_KEY);
    }
  }

  function isValidToken(t) {
    return typeof t === 'string' && /^[A-Z2-7]{26,32}$/.test(t);
  }

  function generateToken() {
    var bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    var alpha = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
    var bits = 0, value = 0, output = '';
    for (var i = 0; i < bytes.length; i++) {
      value = (value << 8) | bytes[i];
      bits += 8;
      while (bits >= 5) {
        output += alpha[(value >>> (bits - 5)) & 31];
        bits -= 5;
      }
    }
    if (bits > 0) {
      output += alpha[(value << (5 - bits)) & 31];
    }
    return output;
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
      console.warn('[Sync] resume write failed:', e);
    }
  }

  function mergeResume(remote) {
    if (!remote || typeof remote !== 'object') return false;
    var local = getResume();
    var merged = {};
    var changed = false;
    var keys = {};
    Object.keys(local).forEach(function (k) { keys[k] = true; });
    Object.keys(remote).forEach(function (k) { keys[k] = true; });
    Object.keys(keys).forEach(function (k) {
      var l = local[k], r = remote[k];
      if (!l) { merged[k] = r; changed = true; return; }
      if (!r) { merged[k] = l; return; }
      var lt = (l && l.updated) || 0;
      var rt = (r && r.updated) || 0;
      if (rt > lt) { merged[k] = r; changed = true; }
      else { merged[k] = l; }
    });
    if (changed) setResume(merged);
    return changed;
  }

  function pull() {
    var token = getToken();
    if (!token) return Promise.resolve(null);
    return fetch('/api/sync/' + encodeURIComponent(token))
      .then(function (res) {
        if (res.status === 404) return null;
        if (!res.ok) throw new Error('HTTP ' + res.status);
        return res.json();
      })
      .then(function (data) {
        if (!data) return null;
        var remoteResume = (data.blob && data.blob.resume) || {};
        var changed = mergeResume(remoteResume);
        try {
          localStorage.setItem(META_KEY, JSON.stringify({
            last_pulled_updated_at: data.updated_at || 0,
            last_pulled_at: Date.now()
          }));
        } catch (e) { /* ignore */ }
        return { changed: changed, updated_at: data.updated_at };
      })
      .catch(function (err) {
        console.warn('[Sync] pull failed:', err);
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
    fetch('/api/sync/' + encodeURIComponent(token), {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: body,
      keepalive: true
    }).catch(function (err) { console.warn('[Sync] push failed:', err); });
  }

  function schedulePush() {
    if (!getToken()) return;
    if (pushTimer) clearTimeout(pushTimer);
    pushTimer = setTimeout(push, DEBOUNCE_MS);
  }

  // localStorage `storage` events fire on OTHER tabs only, so we also poll
  // the resume key in this tab. 2s is fine — the debounce already coalesces.
  var lastResumeStr = localStorage.getItem(RESUME_KEY) || '';
  setInterval(function () {
    var cur = localStorage.getItem(RESUME_KEY) || '';
    if (cur !== lastResumeStr) {
      lastResumeStr = cur;
      schedulePush();
    }
  }, POLL_MS);

  window.addEventListener('storage', function (e) {
    if (e.key === RESUME_KEY) {
      lastResumeStr = e.newValue || '';
      schedulePush();
    }
  });

  // Flush any pending push when the user navigates away. `keepalive: true`
  // on the fetch lets it survive page unload.
  window.addEventListener('beforeunload', function () {
    if (pushTimer) {
      clearTimeout(pushTimer);
      push();
    }
  });

  // Expose for the settings UI (Task 11).
  window.__moonmoonSync = {
    getToken: getToken,
    setToken: setToken,
    isValidToken: isValidToken,
    generateToken: generateToken,
    pull: pull,
    push: push
  };

  // Initial pull on every page load.
  pull();
})();
