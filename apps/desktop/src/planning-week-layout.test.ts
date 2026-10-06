import { describe, expect, it } from "vitest";
import {
  isAllDaySchedule,
  layoutWeekDay,
  weekTimeWindow,
  type WeekTimedItem,
} from "./planning-week-layout";
import type { ScheduleEntry } from "./api/planning";
const day = new Date(2026, 9, 5);
const at = (hour: number, minute = 0, offset = 0) =>
  new Date(2026, 9, 5 + offset, hour, minute).toISOString();
const item = (id: string, start: number, end: number): WeekTimedItem => ({
  id,
  title: id,
  kind: "schedule",
  startsAt: at(start),
  endsAt: at(end),
});
describe("weekly time block layout", () => {
  it("places real local start and end times on the time axis", () => {
    expect(
      layoutWeekDay(
        [{ ...item("meeting", 9, 11), startsAt: at(9, 30) }],
        day,
      )[0],
    ).toMatchObject({
      startMinute: 570,
      endMinute: 660,
      column: 0,
      columns: 1,
    });
  });
  it("splits overlapping connected intervals into lanes and reuses vacant lanes", () => {
    const blocks = layoutWeekDay(
      [
        item("b", 10, 11),
        item("a", 9, 12),
        item("c", 11, 13),
        item("later", 14, 15),
      ],
      day,
    );
    expect(blocks.map((b) => [b.id, b.column, b.columns])).toEqual([
      ["a", 0, 2],
      ["b", 1, 2],
      ["c", 1, 2],
      ["later", 0, 1],
    ]);
  });
  it("does not overlap adjacent meetings", () => {
    expect(
      layoutWeekDay([item("a", 9, 10), item("b", 10, 11)], day).map(
        (b) => b.columns,
      ),
    ).toEqual([1, 1]);
  });
  it("clips overnight events on both days and excludes a midnight end", () => {
    const overnight = { ...item("night", 23, 1), endsAt: at(1, 0, 1) };
    expect(layoutWeekDay([overnight], day)[0]).toMatchObject({
      startMinute: 1380,
      endMinute: 1440,
    });
    expect(layoutWeekDay([overnight], new Date(2026, 9, 6))[0]).toMatchObject({
      startMinute: 0,
      endMinute: 60,
    });
    expect(
      layoutWeekDay(
        [{ ...overnight, endsAt: at(0, 0, 1) }],
        new Date(2026, 9, 6),
      ),
    ).toEqual([]);
  });
  it("preserves received timestamps without claiming a request duration", () => {
    const report: WeekTimedItem = {
      id: "request",
      title: "request",
      kind: "report",
      startsAt: at(11, 15),
    };
    const blocks = layoutWeekDay(
      [report, { ...report, id: "next", startsAt: at(11, 20) }],
      day,
    );
    expect(blocks[0]).toMatchObject({
      startMinute: 675,
      endMinute: 675,
      columns: 2,
    });
    expect(blocks[0].endsAt).toBeUndefined();
  });
  it("ignores invalid, backwards or out-of-day data", () => {
    expect(
      layoutWeekDay(
        [
          { ...item("invalid", 9, 10), startsAt: "bad" },
          item("backwards", 10, 9),
          {
            ...item("tomorrow", 9, 10),
            startsAt: at(9, 0, 1),
            endsAt: at(10, 0, 1),
          },
        ],
        day,
      ),
    ).toEqual([]);
  });
  it("expands visible hours to include early and late events", () => {
    expect(weekTimeWindow([])).toEqual({ startHour: 7, endHour: 20 });
    expect(
      weekTimeWindow(
        layoutWeekDay(
          [
            item("early", 5, 6),
            { ...item("late", 23, 1), endsAt: at(1, 0, 1) },
          ],
          day,
        ),
      ),
    ).toEqual({ startHour: 5, endHour: 24 });
  });
  it("treats midnight to midnight schedules as all-day only with positive duration", () => {
    const entry = { startsAt: at(0), endsAt: at(0, 0, 1) } as ScheduleEntry;
    expect(isAllDaySchedule(entry)).toBe(true);
    expect(isAllDaySchedule({ ...entry, startsAt: at(9) })).toBe(false);
    expect(isAllDaySchedule({ ...entry, endsAt: at(0) })).toBe(false);
  });
});
