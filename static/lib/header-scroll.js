// Scroll-away header policy: hidden while scrolling down past the header,
// revealed on scroll-up, never hidden near the top or while a fullscreen
// overlay owns the viewport. header.js is a thin adapter over this.

// Ignore sub-threshold jitter (momentum wobble, scroll anchoring) so the
// header doesn't flicker at the direction change.
export const SCROLL_JITTER_PX = 4;

/**
 * @param {{
 *   hidden: boolean,
 *   y: number,
 *   lastY: number,
 *   headerHeight: number,
 *   overlayOpen: boolean,
 * }} input
 * @returns {boolean} whether the header should be hidden
 */
export function nextHeaderHidden({ hidden, y, lastY, headerHeight, overlayOpen }) {
  if (overlayOpen) return false;
  if (y <= headerHeight) return false;
  const delta = y - lastY;
  if (delta > SCROLL_JITTER_PX) return true;
  if (delta < -SCROLL_JITTER_PX) return false;
  return hidden;
}
