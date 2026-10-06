import { describe, expect, it } from "vitest";
import type { Task } from "./api/planning";
import {
  filterMobileTasks,
  groupMobileTasks,
  mergeMobileTasks,
} from "./mobileTaskView";

const now = new Date("2026-10-06T15:30:00Z"); // 10/7 00:30 Seoul
const make = (
  id: string,
  dueAt: string | null,
  extra: Partial<Task> = {},
): Task => ({
  id,
  title: id,
  projectId: null,
  notes: null,
  assigneeName: "김경주",
  dueAt,
  priority: 1,
  status: "open",
  completedAt: null,
  version: 1,
  ...extra,
});
const tasks = [
  make("today", "2026-10-06T16:00:00Z"),
  make("overdue-today", "2026-10-06T15:00:00Z"),
  make("yesterday", "2026-10-06T14:00:00Z"),
  make("tomorrow", "2026-10-07T16:00:00Z"),
  make("undated", null),
  make("done", null, { status: "completed" }),
];

describe("mobile task views", () => {
  it("uses Seoul days even at the UTC date boundary", () => {
    expect(filterMobileTasks(tasks, "today", now).map((t) => t.id)).toEqual([
      "overdue-today",
      "today",
    ]);
  });
  it("shows time-overdue work, never completed work", () => {
    expect(filterMobileTasks(tasks, "overdue", now).map((t) => t.id)).toEqual([
      "yesterday",
      "overdue-today",
    ]);
    expect(filterMobileTasks(tasks, "all", now)).toHaveLength(5);
  });
  it("keeps every task without an arbitrary display limit", () => {
    expect(
      filterMobileTasks(
        Array.from({ length: 177 }, (_, i) => make(String(i), null)),
        "all",
        now,
      ),
    ).toHaveLength(177);
  });
  it("keeps multi-assignee work once and unassigned work visible", () => {
    const groups = groupMobileTasks(
      [
        make("a", null, { assigneeName: "송인준, 김경주" }),
        make("b", null, { assigneeName: null }),
      ],
      "assignee",
    );
    expect(groups.map(([name]) => name)).toEqual([
      "송인준, 김경주",
      "담당자 미정",
    ]);
    expect(groups.flatMap(([, items]) => items)).toHaveLength(2);
  });
  it("groups by due day with undated work last", () => {
    expect(
      groupMobileTasks(tasks.slice(0, 5), "date").map(([key]) => key),
    ).toEqual(["2026-10-06", "2026-10-07", "2026-10-08", "기한 없음"]);
  });
  it("never resurrects completed work from an older home response", () => {
    expect(
      mergeMobileTasks(
        [make("a", null, { status: "completed", version: 3 })],
        [make("a", null, { version: 2 })],
      )[0].status,
    ).toBe("completed");
    expect(
      mergeMobileTasks(
        [make("a", null)],
        [make("a", null, { status: "completed", version: 2 })],
      )[0].status,
    ).toBe("completed");
  });
  it("does not re-add completed or deleted IDs absent from the all-open response", () => {
    expect(mergeMobileTasks([], [make("old", null)], true)).toEqual([]);
    expect(mergeMobileTasks([], [make("fallback", null)], false)).toHaveLength(
      1,
    );
  });
});
