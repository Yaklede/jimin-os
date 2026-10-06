import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PlanningSnapshot, ScheduleEntry } from "./api/planning";
import { PlanningCalendar } from "./components/planning-calendar";
import { planningViewRange } from "./planningRange";
const day = new Date(2026, 9, 5);
const at = (hour: number, offset = 0) =>
  new Date(2026, 9, 5 + offset, hour).toISOString();
const schedule = (
  id: string,
  startsAt: string,
  endsAt: string,
): ScheduleEntry => ({
  id,
  title: id,
  startsAt,
  endsAt,
  notes: null,
  timeZone: "Asia/Seoul",
  status: "confirmed",
  source: "manual",
  editable: true,
  version: 1,
});
const snapshot: PlanningSnapshot = {
  schedule: [
    schedule("Timed appointment", at(9), at(11)),
    schedule("All-day appointment", at(0), at(0, 1)),
    {
      ...schedule("Cancelled appointment", at(10), at(12)),
      status: "cancelled",
    },
  ],
  tasks: [
    {
      id: "task",
      title: "Due task",
      projectId: null,
      notes: null,
      status: "open",
      priority: 1,
      dueAt: at(18),
      completedAt: null,
      version: 1,
    },
  ],
  completedTasks: [],
};
describe("weekly calendar content", () => {
  it("keeps tasks and all-day schedules separate from timed appointments and excludes cancelled entries", () => {
    const html = renderToStaticMarkup(
      createElement(PlanningCalendar, {
        range: planningViewRange("week", day),
        snapshot,
        loading: false,
        onSelectDate: () => {},
      }),
    );
    const allDay = html.slice(
      html.indexOf('class="planning-week__all-day"'),
      html.indexOf('class="planning-week__body"'),
    );
    expect(allDay).toContain("Due task");
    expect(allDay).toContain("All-day appointment");
    expect(allDay).not.toContain("Timed appointment");
    expect(html).not.toContain("Cancelled appointment");
    expect(html).toContain(
      'class="planning-week__event focus-visible-control"',
    );
    expect(html).toContain("height:128px");
  });
  it("represents a received request with its receipt time rather than a made-up schedule interval", () => {
    const html = renderToStaticMarkup(
      createElement(PlanningCalendar, {
        range: planningViewRange("week", day),
        snapshot: undefined,
        reports: [
          { id: "request", title: "Received request", receivedAt: at(10) },
        ],
        loading: false,
        onSelectDate: () => {},
      }),
    );
    expect(html).toContain("10:00 받음");
    expect(html).not.toContain("planning-week__all-day");
    expect(html).not.toContain("10:00–");
  });
  it("retains the month date grid instead of adding the time axis", () => {
    const html = renderToStaticMarkup(
      createElement(PlanningCalendar, {
        range: planningViewRange("month", day),
        snapshot,
        loading: false,
        onSelectDate: () => {},
      }),
    );
    expect(html).toContain('class="planning-calendar__grid"');
    expect(html).not.toContain('class="planning-week__body"');
  });
});
