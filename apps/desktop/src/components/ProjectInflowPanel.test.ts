import { describe, expect, it } from "vitest";

import {
  localInputToIso,
  resolvePromotionDeadline,
} from "./ProjectInflowPanel";

describe("project inflow deadline", () => {
  it("keeps a selected local deadline when promoting a Chat request", () => {
    const input = "2026-07-24T18:30";

    expect(localInputToIso(input)).toBe(new Date(input).toISOString());
  });

  it("does not turn an invalid deadline into an empty value", () => {
    expect(localInputToIso("not-a-date")).toBeUndefined();
    expect(localInputToIso("")).toBeUndefined();
  });

  it("requires a deadline unless the user explicitly chooses otherwise", () => {
    expect(resolvePromotionDeadline("", false)).toBeUndefined();
    expect(resolvePromotionDeadline("not-a-date", false)).toBeUndefined();
    expect(resolvePromotionDeadline("", true)).toEqual({
      dueAt: null,
      withoutDeadline: true,
    });
  });

  it("keeps the native date input value in the promotion request", () => {
    const input = "2026-07-29T18:30";

    expect(resolvePromotionDeadline(input, false)).toEqual({
      dueAt: new Date(input).toISOString(),
      withoutDeadline: false,
    });
  });
});
