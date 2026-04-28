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

  // Expose for the settings UI (Task 11).
  window.__moonmoonSync = {
    getToken: getToken,
    setToken: setToken,
    isValidToken: isValidToken,
    generateToken: generateToken,
    pull: pull
  };

  // Initial pull on every page load.
  pull();
})();
