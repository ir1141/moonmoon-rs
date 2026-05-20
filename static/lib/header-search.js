export function nextSearchOverlayState(state, action) {
  const query = typeof state.query === "string" ? state.query : "";

  switch (action.type) {
    case "open":
      return { open: true, query, focusInput: true };
    case "close":
    case "escape":
      return { open: false, query, focusInput: false };
    case "clear":
      return { open: true, query: "", focusInput: true };
    case "backdrop":
      return {
        open: action.onBackdrop ? false : !!state.open,
        query,
        focusInput: false,
      };
    default:
      return { open: !!state.open, query, focusInput: false };
  }
}
