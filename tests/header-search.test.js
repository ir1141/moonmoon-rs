import { describe, expect, test } from "bun:test";
import {
  isEditableTarget,
  isSearchShortcut,
  nextSearchOverlayState,
  nextTrapTarget,
  shouldLockSearchOverlayScroll,
} from "../static/lib/header-search.js";

describe("nextSearchOverlayState", () => {
  test("open shows the overlay and requests input focus", () => {
    expect(
      nextSearchOverlayState(
        { open: false, query: "elden ring" },
        { type: "open" },
      ),
    ).toEqual({
      open: true,
      query: "elden ring",
      focusInput: true,
      focusOpener: false,
    });
  });

  test("close hides the overlay and returns focus to the opener", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "hitman" },
        { type: "close" },
      ),
    ).toEqual({
      open: false,
      query: "hitman",
      focusInput: false,
      focusOpener: true,
    });
  });

  test("clear empties the query and keeps input focus", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "hitman" },
        { type: "clear" },
      ),
    ).toEqual({
      open: true,
      query: "",
      focusInput: true,
      focusOpener: false,
    });
  });

  test("escape closes the overlay while preserving typed input", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "terraria" },
        { type: "escape" },
      ),
    ).toEqual({
      open: false,
      query: "terraria",
      focusInput: false,
      focusOpener: true,
    });
  });

  test("escape on a closed overlay leaves focus alone", () => {
    // Desktop renders the same form as an inline toolbar, where Escape while
    // typing must not fling focus at the hidden opener button.
    expect(
      nextSearchOverlayState(
        { open: false, query: "terraria" },
        { type: "escape" },
      ).focusOpener,
    ).toBe(false);
  });

  test("backdrop taps close only when the tap target is the overlay backdrop", () => {
    expect(
      nextSearchOverlayState(
        { open: true, query: "moon" },
        { type: "backdrop", onBackdrop: true },
      ),
    ).toEqual({
      open: false,
      query: "moon",
      focusInput: false,
      focusOpener: true,
    });
    expect(
      nextSearchOverlayState(
        { open: true, query: "moon" },
        { type: "backdrop", onBackdrop: false },
      ),
    ).toEqual({
      open: true,
      query: "moon",
      focusInput: false,
      focusOpener: false,
    });
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

describe("isEditableTarget", () => {
  test("form controls and contenteditable count as editable", () => {
    expect(isEditableTarget({ tagName: "INPUT" })).toBe(true);
    expect(isEditableTarget({ tagName: "TEXTAREA" })).toBe(true);
    expect(isEditableTarget({ tagName: "SELECT" })).toBe(true);
    expect(isEditableTarget({ tagName: "DIV", isContentEditable: true })).toBe(
      true,
    );
  });

  test("everything else does not", () => {
    expect(isEditableTarget({ tagName: "BUTTON" })).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
});

describe("isSearchShortcut", () => {
  const slash = (over) => ({ key: "/", target: { tagName: "BODY" }, ...over });

  test("slash outside a text field focuses search", () => {
    expect(isSearchShortcut(slash())).toBe(true);
  });

  test("slash yields to text entry", () => {
    expect(isSearchShortcut(slash({ target: { tagName: "INPUT" } }))).toBe(
      false,
    );
  });

  test("slash with a modifier belongs to the browser", () => {
    expect(isSearchShortcut(slash({ ctrlKey: true }))).toBe(false);
    expect(isSearchShortcut(slash({ metaKey: true }))).toBe(false);
    expect(isSearchShortcut(slash({ altKey: true }))).toBe(false);
  });

  test("ctrl/cmd-k fires even from inside a text field", () => {
    expect(
      isSearchShortcut({ key: "k", ctrlKey: true, target: { tagName: "INPUT" } }),
    ).toBe(true);
    expect(
      isSearchShortcut({ key: "K", metaKey: true, target: { tagName: "INPUT" } }),
    ).toBe(true);
  });

  test("bare k is just a letter", () => {
    expect(isSearchShortcut({ key: "k", target: { tagName: "BODY" } })).toBe(
      false,
    );
  });
});

describe("nextTrapTarget", () => {
  const items = ["first", "middle", "last"];

  test("wraps at the ends so focus cannot leave the sheet", () => {
    expect(nextTrapTarget(items, "last", false)).toBe("first");
    expect(nextTrapTarget(items, "first", true)).toBe("last");
  });

  test("leaves the interior to the browser", () => {
    // Chrome walks the sub-fields of a date input with Tab; hijacking every Tab
    // would strand the user on the month segment.
    expect(nextTrapTarget(items, "middle", false)).toBeNull();
    expect(nextTrapTarget(items, "middle", true)).toBeNull();
    expect(nextTrapTarget(items, "first", false)).toBeNull();
    expect(nextTrapTarget(items, "last", true)).toBeNull();
  });

  test("an empty overlay has nothing to trap", () => {
    expect(nextTrapTarget([], "first", false)).toBeNull();
  });
});
