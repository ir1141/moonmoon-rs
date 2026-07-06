import { describe, expect, test } from "bun:test";
import { nextHeaderHidden, SCROLL_JITTER_PX } from "./header-scroll.js";

const base = { hidden: false, y: 500, lastY: 500, headerHeight: 110, overlayOpen: false };

describe("nextHeaderHidden", () => {
  test("hides on a clear scroll down past the header", () => {
    expect(nextHeaderHidden({ ...base, y: 540, lastY: 500 })).toBe(true);
  });

  test("reveals on a clear scroll up", () => {
    expect(nextHeaderHidden({ ...base, hidden: true, y: 460, lastY: 500 })).toBe(false);
  });

  test("keeps current state within the jitter threshold", () => {
    const wobble = SCROLL_JITTER_PX;
    expect(nextHeaderHidden({ ...base, hidden: true, y: 500 + wobble, lastY: 500 })).toBe(true);
    expect(nextHeaderHidden({ ...base, hidden: false, y: 500 - wobble, lastY: 500 })).toBe(false);
  });

  test("never hides near the top of the page", () => {
    expect(nextHeaderHidden({ ...base, hidden: true, y: 100, lastY: 60 })).toBe(false);
    expect(nextHeaderHidden({ ...base, y: 0, lastY: 0 })).toBe(false);
  });

  test("never hides while an overlay owns the viewport", () => {
    expect(nextHeaderHidden({ ...base, hidden: true, y: 540, lastY: 500, overlayOpen: true })).toBe(false);
  });
});
