import { describe, expect, it } from "vitest";

import { type ScheduleEntry } from "./api/planning";
import {
  localDayKey,
  millisecondsUntilNextLocalDay,
  upcomingHomeSchedules,
} from "./homeSchedule";

function schedule(
  id: string,
  startsAt: Date,
  endsAt: Date,
  status: ScheduleEntry["status"] = "confirmed",
): ScheduleEntry {
  return {
    id,
    title: id,
    notes: null,
    startsAt: startsAt.toISOString(),
    endsAt: endsAt.toISOString(),
    timeZone: "Asia/Seoul",
    status,
    source: "manual",
    editable: true,
    version: 1,
  };
}

describe("home schedule selection", () => {
  const now = new Date(2026, 6, 15, 12, 0, 0);

  it("removes ended and cancelled entries before choosing the next schedule", () => {
    const entries = [
      schedule("ended", new Date(2026, 6, 15, 9), new Date(2026, 6, 15, 10)),
      schedule(
        "cancelled",
        new Date(2026, 6, 15, 13),
        new Date(2026, 6, 15, 14),
        "cancelled",
      ),
      schedule(
        "upcoming",
        new Date(2026, 6, 15, 15),
        new Date(2026, 6, 15, 16),
      ),
    ];

    expect(upcomingHomeSchedules(entries, now).map(({ id }) => id)).toEqual([
      "upcoming",
    ]);
  });

  it("keeps an in-progress entry and sorts remaining entries by start time", () => {
    const entries = [
      schedule("later", new Date(2026, 6, 15, 18), new Date(2026, 6, 15, 19)),
      schedule(
        "in-progress",
        new Date(2026, 6, 15, 11),
        new Date(2026, 6, 15, 13),
      ),
      schedule("sooner", new Date(2026, 6, 15, 14), new Date(2026, 6, 15, 15)),
    ];

    expect(upcomingHomeSchedules(entries, now).map(({ id }) => id)).toEqual([
      "in-progress",
      "sooner",
      "later",
    ]);
  });

  it("calculates the local-day rollover without a UTC date assumption", () => {
    const now = new Date(2026, 6, 15, 23, 59, 30);

    expect(localDayKey(now)).toBe("2026-7-15");
    expect(millisecondsUntilNextLocalDay(now)).toBe(30_000);
  });
});
