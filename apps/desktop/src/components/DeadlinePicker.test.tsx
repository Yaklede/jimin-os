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
  it("can collapse shortcuts while keeping the date and time controls available", () => {
    const markup = renderToStaticMarkup(
      createElement(DeadlinePicker, {
        id: "compact-deadline",
        label: "마감",
        value: "",
        onChange: () => undefined,
        showPresets: true,
        collapsePresets: true,
      }),
    );
    expect(markup).toContain('id="compact-deadline-date"');
    expect(markup).toContain('<details class="deadline-picker__shortcuts">');
    expect(markup).toContain(deadlinePickerCopy.todayThree);
  });
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
    expect(markup).toContain(deadlinePickerCopy.todayTen);
    expect(markup).toContain(deadlinePickerCopy.todayThree);
    expect(markup).toContain(deadlinePickerCopy.todaySix);
    expect(markup).toContain(deadlinePickerCopy.tomorrowSix);
  });

  it.each([
    ["2026-07-31T10:00", deadlinePickerCopy.todayTen],
    ["2026-07-31T15:00", deadlinePickerCopy.todayThree],
  ])("marks the exact Seoul quick time for %s", (value, label) => {
    // July 30 UTC is already July 31 in Korea.
    const markup = renderToStaticMarkup(
      createElement(DeadlinePicker, {
        id: "deadline",
        label: "마감",
        value,
        now: new Date("2026-07-30T16:00:00Z"),
        onChange: () => undefined,
        showPresets: true,
      }),
    );
    const pressedButtons = markup.match(/aria-pressed="true">.*?<\/button>/g);
    expect(pressedButtons).toHaveLength(1);
    expect(pressedButtons?.[0]).toContain(`<span>${label}</span>`);
  });

  it("keeps minute options in order for precise keyboard stepping", () => {
    const markup = renderToStaticMarkup(
      createElement(DeadlinePicker, {
        id: "deadline",
        label: "마감",
        value: "2026-07-31T15:31",
        onChange: () => undefined,
      }),
    );
    const minutes = markup
      .split('id="deadline-minute"')[1]
      .split("</select>")[0];
    const values = Array.from(
      minutes.matchAll(/<option value="(\d{2})"/g),
      (match) => match[1],
    );
    expect(values).toEqual(
      Array.from({ length: 60 }, (_, minute) =>
        String(minute).padStart(2, "0"),
      ),
    );
    expect(minutes).toContain('value="31" selected');
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
