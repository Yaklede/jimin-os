import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { deadlinePickerCopy } from "../copy/deadlinePicker";
import {
  DeadlinePicker,
  formatSeoulDateTimePreview,
  isoToSeoulLocalDateTime,
  resolveOptionalSeoulDateTime,
  seoulLocalDateTimeToIso,
  selectLocalTimePart,
} from "./DeadlinePicker";

describe("DeadlinePicker", () => {
  it("offers separate date, hour, and every minute without an unsupported native time picker", () => {
    const markup = renderToStaticMarkup(
      createElement(DeadlinePicker, {
        id: "deadline",
        label: "마감",
        value: "2026-07-31T18:30",
        onChange: () => undefined,
        showPresets: true,
      }),
    );

    expect(markup).toContain('type="date"');
    expect(markup).not.toContain('type="time"');
    expect(markup).not.toContain('step="900"');
    expect(markup).toContain('aria-label="마감 시"');
    expect(markup).toContain('aria-label="마감 분"');
    expect(markup).toContain('value="59"');
    expect(markup).toContain(deadlinePickerCopy.todaySix);
    expect(markup).toContain(deadlinePickerCopy.tomorrowSix);
  });

  it("changes hour or minute without dropping the date or rounding a saved time", () => {
    expect(selectLocalTimePart("2026-10-06T14:42", "hour", "16")).toBe(
      "2026-10-06T16:42",
    );
    expect(selectLocalTimePart("2026-10-06T14:42", "minute", "59")).toBe(
      "2026-10-06T14:59",
    );
    expect(
      selectLocalTimePart("", "hour", "18", new Date("2026-10-06T01:00:00Z")),
    ).toBe("2026-10-06T18:00");
    expect(selectLocalTimePart("2026-10-06T14:42", "hour", "24")).toBe(
      "2026-10-06T14:42",
    );
  });

  it("converts the selected Korean time independently of the device zone", () => {
    expect(seoulLocalDateTimeToIso("2026-07-31T18:30")).toBe(
      "2026-07-31T09:30:00.000Z",
    );
    expect(isoToSeoulLocalDateTime("2026-07-31T09:30:00.000Z")).toBe(
      "2026-07-31T18:30",
    );
  });

  it("rejects incomplete or impossible values", () => {
    expect(seoulLocalDateTimeToIso("")).toBeUndefined();
    expect(seoulLocalDateTimeToIso("2026-02-30T18:30")).toBeUndefined();
    expect(formatSeoulDateTimePreview("2026-07-31T")).toBe(
      deadlinePickerCopy.empty,
    );
  });

  it("distinguishes an intentionally empty deadline from a partial value", () => {
    expect(resolveOptionalSeoulDateTime("")).toEqual({
      valid: true,
      value: undefined,
    });
    expect(resolveOptionalSeoulDateTime("2026-07-31T")).toEqual({
      valid: false,
      value: undefined,
    });
    expect(resolveOptionalSeoulDateTime("2026-07-31T18:30")).toEqual({
      valid: true,
      value: "2026-07-31T09:30:00.000Z",
    });
  });
});
