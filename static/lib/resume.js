export function mergeResume(local, remote) {
  const safeLocal = local && typeof local === "object" ? local : {};
  if (!remote || typeof remote !== "object") {
    return { merged: { ...safeLocal }, changed: false };
  }
  const merged = {};
  let changed = false;
  const keys = new Set([...Object.keys(safeLocal), ...Object.keys(remote)]);
  for (const k of keys) {
    const l = safeLocal[k];
    const r = remote[k];
    if (!l && !r) {
      // Junk entry (null/0/false) on one or both sides: drop it rather than
      // copying an undefined value into the merged map — JSON.stringify would
      // silently delete the key from the synced blob. Flag a change only when
      // the junk came from local storage so it gets cleaned up there.
      if (Object.prototype.hasOwnProperty.call(safeLocal, k)) changed = true;
      continue;
    }
    if (!l) {
      merged[k] = r;
      changed = true;
      continue;
    }
    if (!r) {
      merged[k] = l;
      continue;
    }
    const lt = l.updated || 0;
    const rt = r.updated || 0;
    if (rt > lt) {
      merged[k] = r;
      changed = true;
    } else {
      merged[k] = l;
    }
  }
  return { merged, changed };
}
