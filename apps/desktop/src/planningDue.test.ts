import { describe, expect, it } from "vitest";

import { type Task } from "./api/planning";
import { deadlineAttentionTasks, taskDueState } from "./planningDue";

const now = new Date(2026, 6, 14, 12, 0, 0);

function task(id: string, dueAt: Date | null): Task {
  return {
    id,
    projectId: null,
    title: id,
    notes: null,
    status: "open",
    priority: 1,
    dueAt: dueAt?.toISOString() ?? null,
    completedAt: null,
    version: 1,
  };
}

describe("task deadline attention", () => {
  it("distinguishes overdue, today, tomorrow, and later deadlines", () => {
    expect(taskDueState(task("overdue", new Date(2026, 6, 14, 11)), now)).toBe(
      "overdue",
    );
    expect(taskDueState(task("today", new Date(2026, 6, 14, 18)), now)).toBe(
      "today",
    );
    expect(taskDueState(task("tomorrow", new Date(2026, 6, 15, 18)), now)).toBe(
      "tomorrow",
    );
    expect(taskDueState(task("later", new Date(2026, 6, 16, 18)), now)).toBe(
      "later",
    );
    expect(taskDueState(task("undated", null), now)).toBe("none");
  });

  it("keeps only actionable deadlines and sorts the most urgent first", () => {
    const tasks = [
      task("tomorrow", new Date(2026, 6, 15, 9)),
      task("later", new Date(2026, 6, 18, 9)),
      task("today", new Date(2026, 6, 14, 17)),
      task("overdue", new Date(2026, 6, 13, 17)),
      task("undated", null),
    ];

    expect(deadlineAttentionTasks(tasks, now).map(({ id }) => id)).toEqual([
      "overdue",
      "today",
      "tomorrow",
    ]);
  });
});
