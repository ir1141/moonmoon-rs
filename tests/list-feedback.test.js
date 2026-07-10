import { describe, expect, test } from "bun:test";
import {
  overlayApplyState,
  pruneEmptyListParameters,
} from "../static/lib/list-feedback.js";

describe("overlayApplyState", () => {
  test("mirrors the live count onto the dismiss button", () => {
    expect(overlayApplyState("71 streams")).toEqual({
      label: "Show 71 streams",
      empty: false,
    });
    expect(overlayApplyState("511 games")).toEqual({
      label: "Show 511 games",
      empty: false,
    });
  });

  test("keeps the server's singular noun", () => {
    expect(overlayApplyState("1 stream")).toEqual({
      label: "Show 1 stream",
      empty: false,
    });
  });

  test("zero results read as a finding, not an invitation", () => {
    expect(overlayApplyState("0 streams")).toEqual({
      label: "No streams match",
      empty: true,
    });
  });

  test("an unparseable label falls back to a safe dismiss", () => {
    expect(overlayApplyState("")).toEqual({ label: "Show results", empty: false });
    expect(overlayApplyState(null)).toEqual({ label: "Show results", empty: false });
    expect(overlayApplyState("streams")).toEqual({
      label: "Show results",
      empty: false,
    });
  });
});

describe("pruneEmptyListParameters", () => {
  test("removes empty optional list parameters from FormData", () => {
    const parameters = new FormData();
    parameters.set("search", "   ");
    parameters.set("from", "");
    parameters.set("to", "");
    parameters.set("page", "");
    parameters.set("sort", "most");

    pruneEmptyListParameters(parameters);

    expect(parameters.has("search")).toBe(false);
    expect(parameters.has("from")).toBe(false);
    expect(parameters.has("to")).toBe(false);
    expect(parameters.has("page")).toBe(false);
    expect(parameters.get("sort")).toBe("most");
  });

  test("keeps non-empty list parameters", () => {
    const parameters = {
      search: "hitman",
      from: "2026-05-01",
      to: "2026-05-31",
      page: "2",
      sort: "fewest",
    };

    pruneEmptyListParameters(parameters);

    expect(parameters).toEqual({
      search: "hitman",
      from: "2026-05-01",
      to: "2026-05-31",
      page: "2",
      sort: "fewest",
    });
  });
});
