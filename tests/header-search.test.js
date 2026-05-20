import { describe, expect, test } from "bun:test";
import {
  nextSearchOverlayState,
  shouldLockSearchOverlayScroll,
} from "../static/lib/header-search.js";

describe("nextSearchOverlayState", () => {
  test("open shows the overlay and requests input focus", () => {
    expect(
      nextSearchOverlayState(
        { open: false, query: "elden ring" },
        { type: "open" },
      ),
    ).toEqual({ open: true, query: "elden ring", focusInput: true });
  });

  test("close hides the overlay without clearing typed input", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "hitman" },
        { type: "close" },
      ),
    ).toEqual({ open: false, query: "hitman", focusInput: false });
  });

  test("clear empties the query and keeps input focus", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "hitman" },
        { type: "clear" },
      ),
    ).toEqual({ open: true, query: "", focusInput: true });
  });

  test("escape closes the overlay while preserving typed input", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "terraria" },
        { type: "escape" },
      ),
    ).toEqual({ open: false, query: "terraria", focusInput: false });
  });

  test("backdrop taps close only when the tap target is the overlay backdrop", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "moon" },
        { type: "backdrop", onBackdrop: true },
      ),
    ).toEqual({ open: false, query: "moon", focusInput: false });
    expect(
      nextSearchOverlayState(
        { open: true, query: "moon" },
        { type: "backdrop", onBackdrop: false },
      ),
    ).toEqual({ open: true, query: "moon", focusInput: false });
  });

  test("body scroll lock applies only while an overlay is open on mobile", () => {
    expect(shouldLockSearchOverlayScroll({ open: true, mobile: true })).toBe(
      true,
    );
    expect(shouldLockSearchOverlayScroll({ open: true, mobile: false })).toBe(
      false,
    );
    expect(shouldLockSearchOverlayScroll({ open: false, mobile: true })).toBe(
      false,
    );
  });
});
