import { describe, expect, it } from "vitest";

import {
  defaultScheduleRange,
  validateScheduleTimes,
} from "./PlanningCreateDialog";

describe("planning create dialog", () => {
  it("prepares the next half-hour as a one-hour schedule", () => {
    expect(defaultScheduleRange(new Date(2026, 6, 15, 10, 8))).toEqual({
      startsAt: "2026-07-15T10:30",
      endsAt: "2026-07-15T11:30",
    });
    expect(defaultScheduleRange(new Date(2026, 6, 15, 10, 45))).toEqual({
      startsAt: "2026-07-15T11:00",
      endsAt: "2026-07-15T12:00",
    });
  });

  it("requires both schedule times and a later end time", () => {
    expect(validateScheduleTimes("", "2026-07-15T11:00")).toBe(
      "시작 시간과 종료 시간을 모두 입력해 주세요.",
    );
    expect(validateScheduleTimes("2026-07-15T11:00", "2026-07-15T10:30")).toBe(
      "종료 시간은 시작 시간보다 늦어야 해요.",
    );
    expect(
      validateScheduleTimes("2026-07-15T10:00", "2026-07-15T11:00"),
    ).toBeUndefined();
  });
});
