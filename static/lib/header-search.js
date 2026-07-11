export function nextSearchOverlayState(state, action) {
  const query = typeof state.query === "string" ? state.query : "";
  const open = !!state.open;

  switch (action.type) {
    case "open":
      return { open: true, query, focusInput: true, focusOpener: false };
    case "close":
    case "escape":
      return { open: false, query, focusInput: false, focusOpener: open };
    case "clear":
      return { open: true, query: "", focusInput: true, focusOpener: false };
    case "backdrop": {
      const next = action.onBackdrop ? false : open;
      return {
        open: next,
        query,
        focusInput: false,
        focusOpener: open && !next,
      };
    }
    default:
      return { open, query, focusInput: false, focusOpener: false };
  }
}

export function shouldLockSearchOverlayScroll(state) {
  return !!state.open && !!state.mobile;
}

export function isEditableTarget(target) {
  if (!target) return false;
  if (target.isContentEditable) return true;
  const tag = String(target.tagName || "").toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select";
}

/**
 * `/` focuses search the way it does on GitHub and YouTube; Ctrl/Cmd-K is the
 * command-palette reflex. `/` yields to text entry, Ctrl/Cmd-K never does.
 */
export function isSearchShortcut(event) {
  if (event.altKey) return false;
  const key = String(event.key || "");
  if ((event.ctrlKey || event.metaKey) && key.toLowerCase() === "k")
    return true;
  if (key !== "/" || event.ctrlKey || event.metaKey) return false;
  return !isEditableTarget(event.target);
}

/**
 * The element Tab should wrap to, or null when the browser's own Tab handling
 * already keeps focus inside. Returning null on the interior stops us from
 * hijacking Tab between the sub-fields of a date input.
 */
export function nextTrapTarget(items, active, shift) {
  if (!items.length) return null;
  const first = items[0];
  const last = items[items.length - 1];
  if (shift && active === first) return last;
  if (!shift && active === last) return first;
  return null;
}
