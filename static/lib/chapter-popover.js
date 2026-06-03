export function nextChapterPopoverOpen(isOpen, action) {
  switch (action.type) {
    case "chip":
      return !isOpen;
    case "outside":
    case "escape":
      return false;
    case "inside":
    default:
      return isOpen;
  }
}
