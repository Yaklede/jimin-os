import { describe, expect, it } from "vitest";
import { designPreviewInflow } from "./design-preview-inflow";
import {
  formatHomeInflowAge,
  homeInflowByReceivedDate,
  homeInflowOnDate,
} from "./home-inflow-dates";

describe("home incoming report dates", () => {
  const day = new Date(2026, 9, 2);
  it("uses received day, including local-day edges, instead of task deadline", () => {
    const base = designPreviewInflow(day)[0];
    const reports = [
      {
        ...base,
        id: "start",
        receivedAt: new Date(2026, 9, 2, 0, 0).toISOString(),
        suggestedDueAt: new Date(2026, 9, 20).toISOString(),
      },
      {
        ...base,
        id: "end",
        receivedAt: new Date(2026, 9, 2, 23, 59, 59).toISOString(),
      },
      {
        ...base,
        id: "next",
        receivedAt: new Date(2026, 9, 3, 0, 0).toISOString(),
      },
      { ...base, id: "unknown", receivedAt: "invalid" },
    ];
    expect(homeInflowOnDate(reports, day).map((item) => item.id)).toEqual([
      "start",
      "end",
    ]);
  });
  it("keeps every request available and sorts newest first without changing input", () => {
    const reports = designPreviewInflow(day).reverse();
    const firstId = reports[0].id;
    const sorted = homeInflowByReceivedDate(reports);
    expect(sorted).toHaveLength(26);
    expect(reports[0].id).toBe(firstId);
    expect(new Date(sorted[0].receivedAt).getTime()).toBeGreaterThan(
      new Date(sorted[25].receivedAt).getTime(),
    );
    expect(homeInflowOnDate(sorted, day)).toHaveLength(4);
    expect(homeInflowOnDate(sorted, new Date(2026, 10, 1))).toEqual([]);
  });
  it("keeps reports with unknown dates in the full list", () => {
    const reports = designPreviewInflow(day).slice(0, 2);
    reports[0].receivedAt = "unknown";
    expect(homeInflowByReceivedDate(reports)[1].id).toBe(reports[0].id);
  });
});

describe("home incoming report age", () => {
  const nowMs = Date.parse("2026-11-26T05:00:00Z");

  it.each([
    [0, "방금"],
    [35, "35분 전"],
    [60, "1시간 전"],
    [23 * 60 + 59, "23시간 전"],
    [24 * 60, "1일 전"],
    [7 * 24 * 60 + 59, "7일 전"],
    [30 * 24 * 60, "30일 전"],
  ])("formats %s elapsed minutes as %s", (minutes, expected) => {
    expect(formatHomeInflowAge(nowMs - minutes * 60_000, nowMs)).toBe(expected);
  });

  it("does not invent an age for unknown or future timestamps", () => {
    expect(formatHomeInflowAge(NaN, nowMs)).toBeUndefined();
    expect(formatHomeInflowAge(nowMs + 1, nowMs)).toBeUndefined();
  });
});
