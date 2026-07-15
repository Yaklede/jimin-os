import { describe, expect, it } from "vitest";

import {
  currentPlanningRange,
  planningViewRange,
  samePlanningViewRange,
  shiftPlanningViewRange,
} from "./planningRange";

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

describe("planning view range", () => {
  it("builds local day, Monday-based week, and month ranges", () => {
    expect(planningViewRange("day", now)).toMatchObject({
      from: new Date(2026, 6, 14),
      to: new Date(2026, 6, 15),
    });
    expect(planningViewRange("week", now)).toMatchObject({
      from: new Date(2026, 6, 13),
      to: new Date(2026, 6, 20),
    });
    expect(planningViewRange("month", now)).toMatchObject({
      from: new Date(2026, 6, 1),
      to: new Date(2026, 7, 1),
    });
  });

  it("treats separately-created ranges for the same view as equal", () => {
    const anchor = new Date(2026, 6, 15, 14, 30);

    expect(
      samePlanningViewRange(
        planningViewRange("month", anchor),
        planningViewRange("month", new Date(anchor)),
      ),
    ).toBe(true);
    expect(
      samePlanningViewRange(
        planningViewRange("month", anchor),
        planningViewRange("month", new Date(2026, 7, 1)),
      ),
    ).toBe(false);
  });

  it("moves a month range without limiting older history", () => {
    const range = planningViewRange("month", now);
    const previous = shiftPlanningViewRange(range, -1);
    const older = shiftPlanningViewRange(previous, -1);

    expect(previous.from).toEqual(new Date(2026, 5, 1));
    expect(older.from).toEqual(new Date(2026, 4, 1));
  });
});
