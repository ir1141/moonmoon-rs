import { describe, expect, test } from "bun:test";
import { pruneEmptyListParameters } from "../static/lib/list-feedback.js";

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
