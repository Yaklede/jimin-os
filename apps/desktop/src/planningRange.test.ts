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

import {
  planningCalendarDays,
  samePlanningDay,
  scheduleOverlapsPlanningDay,
} from "./planningRange";

describe("planning calendar dates", () => {
  it("shows every leap-February date in complete Monday-based weeks", () => {
    const range = planningViewRange("month", new Date(2028, 1, 15));
    const days = planningCalendarDays(range);
    expect(days.length % 7).toBe(0);
    expect(days[0].getDay()).toBe(1);
    expect(days.at(-1)?.getDay()).toBe(0);
    const monthDays = days.filter((day) => day >= range.from && day < range.to);
    expect(monthDays).toHaveLength(29);
    expect(monthDays[0].getDate()).toBe(1);
    expect(monthDays.at(-1)?.getDate()).toBe(29);
  });
  it("shows exactly one week across a year boundary", () => {
    const range = planningViewRange("week", new Date(2027, 0, 1));
    const days = planningCalendarDays(range);
    expect(days).toHaveLength(7);
    expect(days[0]).toEqual(new Date(2026, 11, 28));
    expect(days[6]).toEqual(new Date(2027, 0, 3));
    expect(new Set(days.map((day) => day.getTime())).size).toBe(7);
  });
  it("uses calendar-day equality instead of equal timestamps", () => {
    expect(
      samePlanningDay(new Date(2026, 9, 2, 0), new Date(2026, 9, 2, 23)),
    ).toBe(true);
    expect(samePlanningDay(new Date(2026, 9, 2), new Date(2026, 9, 3))).toBe(
      false,
    );
  });
  it("includes overnight schedules on each affected date but excludes midnight end", () => {
    const entry = {
      startsAt: new Date(2026, 9, 2, 23).toISOString(),
      endsAt: new Date(2026, 9, 3, 1).toISOString(),
    };
    expect(scheduleOverlapsPlanningDay(entry, new Date(2026, 9, 2))).toBe(true);
    expect(scheduleOverlapsPlanningDay(entry, new Date(2026, 9, 3))).toBe(true);
    expect(scheduleOverlapsPlanningDay(entry, new Date(2026, 9, 4))).toBe(
      false,
    );
    expect(
      scheduleOverlapsPlanningDay(
        { ...entry, endsAt: new Date(2026, 9, 3).toISOString() },
        new Date(2026, 9, 3),
      ),
    ).toBe(false);
  });
});
