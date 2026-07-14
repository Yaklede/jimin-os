import { describe, expect, it } from "vitest";

import { currentPlanningRange } from "./planningRange";

const now = new Date(2026, 6, 14, 12, 0, 0);

describe("planning range", () => {
  it("includes recent history and the next ninety days by default", () => {
    const [from, to] = currentPlanningRange(undefined, now);

    expect(from).toEqual(new Date(2026, 3, 14));
    expect(to).toEqual(new Date(2026, 9, 12));
  });

  it("extends backward to an explicitly selected older schedule", () => {
    const target = new Date(2025, 11, 20, 15).toISOString();
    const [from, to] = currentPlanningRange(target, now);

    expect(from).toEqual(new Date(2025, 11, 20));
    expect(to).toEqual(new Date(2026, 9, 12));
  });

  it("extends forward to an explicitly selected future schedule", () => {
    const target = new Date(2027, 0, 5, 15).toISOString();
    const [from, to] = currentPlanningRange(target, now);

    expect(from).toEqual(new Date(2026, 3, 14));
    expect(to).toEqual(new Date(2027, 0, 6));
  });
});
