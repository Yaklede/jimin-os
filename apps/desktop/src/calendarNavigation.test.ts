import { describe, expect, it } from "vitest";

import {
  calendarDestinationActivation,
  calendarDestinationLoad,
} from "./calendarNavigation";

describe("calendar destination loading", () => {
  it("loads the current range for direct and native navigation", () => {
    expect(calendarDestinationLoad()).toEqual({
      shouldLoadPlanning: true,
      targetStartsAt: undefined,
    });
  });

  it("loads the range that contains the assistant schedule", () => {
    expect(
      calendarDestinationLoad({
        planningReady: false,
        targetStartsAt: "2026-08-14T09:00:00+09:00",
      }),
    ).toEqual({
      shouldLoadPlanning: true,
      targetStartsAt: "2026-08-14T09:00:00+09:00",
    });
  });

  it("does not reload planning data that was fetched before navigation", () => {
    expect(
      calendarDestinationLoad({
        planningReady: true,
        targetStartsAt: "2026-08-14T09:00:00+09:00",
      }),
    ).toEqual({ shouldLoadPlanning: false });
  });

  it("loads once per calendar visit even when effect dependencies change", () => {
    const firstVisit = calendarDestinationActivation(false, true);
    expect(firstVisit).toEqual({ active: true, shouldLoad: true });

    const repeatedEffect = calendarDestinationActivation(
      firstVisit.active,
      true,
    );
    expect(repeatedEffect).toEqual({ active: true, shouldLoad: false });

    const leftCalendar = calendarDestinationActivation(
      repeatedEffect.active,
      false,
    );
    expect(leftCalendar).toEqual({ active: false, shouldLoad: false });
    expect(calendarDestinationActivation(leftCalendar.active, true)).toEqual({
      active: true,
      shouldLoad: true,
    });
  });
});
