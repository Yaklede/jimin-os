import { describe, expect, it } from "vitest";

import { homeBriefPeriodForHour } from "./homeBriefPeriod";

describe("homeBriefPeriodForHour", () => {
  it.each([
    [0, "morning"],
    [11, "morning"],
    [12, "daytime"],
    [17, "daytime"],
    [18, "evening"],
    [23, "evening"],
  ] as const)("maps %i:00 to %s", (hour, expected) => {
    expect(homeBriefPeriodForHour(hour)).toBe(expected);
  });
});
