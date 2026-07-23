import { describe, expect, it } from "vitest";

import { localInputToIso } from "./ProjectInflowPanel";

describe("project inflow deadline", () => {
  it("keeps a selected local deadline when promoting a Chat request", () => {
    const input = "2026-07-24T18:30";

    expect(localInputToIso(input)).toBe(new Date(input).toISOString());
  });

  it("does not turn an invalid deadline into an empty value", () => {
    expect(localInputToIso("not-a-date")).toBeUndefined();
    expect(localInputToIso("")).toBeUndefined();
  });
});
