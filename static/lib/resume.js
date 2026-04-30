export function mergeResume(local, remote) {
  const safeLocal = (local && typeof local === 'object') ? local : {};
  if (!remote || typeof remote !== 'object') {
    return { merged: { ...safeLocal }, changed: false };
  }
  const merged = {};
  let changed = false;
  const keys = new Set([...Object.keys(safeLocal), ...Object.keys(remote)]);
  for (const k of keys) {
    const l = safeLocal[k];
    const r = remote[k];
    if (!l) { merged[k] = r; changed = true; continue; }
    if (!r) { merged[k] = l; continue; }
    const lt = (l && l.updated) || 0;
    const rt = (r && r.updated) || 0;
    if (rt > lt) { merged[k] = r; changed = true; }
    else { merged[k] = l; }
  }
  return { merged, changed };
}
