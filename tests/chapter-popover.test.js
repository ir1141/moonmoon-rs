import { describe, expect, test } from "bun:test";
import { nextChapterPopoverOpen } from "../static/lib/chapter-popover.js";

describe("nextChapterPopoverOpen", () => {
  test("chip clicks toggle the popover", () => {
    expect(nextChapterPopoverOpen(false, { type: "chip" })).toBe(true);
    expect(nextChapterPopoverOpen(true, { type: "chip" })).toBe(false);
  });

  test("outside clicks and escape close the popover", () => {
    expect(nextChapterPopoverOpen(true, { type: "outside" })).toBe(false);
    expect(nextChapterPopoverOpen(true, { type: "escape" })).toBe(false);
  });

  test("inside clicks preserve the current state", () => {
    expect(nextChapterPopoverOpen(true, { type: "inside" })).toBe(true);
    expect(nextChapterPopoverOpen(false, { type: "inside" })).toBe(false);
  });
});
