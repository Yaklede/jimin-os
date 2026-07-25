import { describe, expect, it } from "vitest";

import { type Task } from "../api/planning";
import { type WeeklyReport } from "../api/projects";
import { selectWeeklyPriorityTasks } from "./HomeWorkspace";

function task(
  id: string,
  projectId: string | null,
  status: Task["status"] = "open",
): Task {
  return {
    id,
    projectId,
    title: id,
    notes: null,
    status,
    priority: 2,
    dueAt: "2026-07-25T09:00:00Z",
    completedAt: null,
    version: 1,
  };
}

function report(): WeeklyReport {
  return {
    workspaceId: "workspace-company",
    periodStart: "2026-07-20",
    periodEnd: "2026-07-26",
    createdTaskCount: 5,
    completedTaskCount: 2,
    backlogStartCount: 4,
    backlogEndCount: 7,
    backlogDelta: 3,
    overdueTaskCount: 2,
    staleTaskCount: 1,
    unassignedTaskCount: 0,
    projects: [
      {
        projectId: "project-attention",
        title: "확인이 필요한 프로젝트",
        managementMode: "operation",
        createdTaskCount: 4,
        completedTaskCount: 1,
        backlogStartCount: 3,
        backlogEndCount: 6,
        backlogDelta: 3,
        overdueTaskCount: 2,
        staleTaskCount: 1,
        unassignedTaskCount: 0,
        averageCycleTimeHours: 8,
        onTimeCompletionPercent: 50,
        health: "needs_attention",
      },
      {
        projectId: "project-on-track",
        title: "순조로운 프로젝트",
        managementMode: "completion",
        createdTaskCount: 1,
        completedTaskCount: 1,
        backlogStartCount: 1,
        backlogEndCount: 1,
        backlogDelta: 0,
        overdueTaskCount: 0,
        staleTaskCount: 0,
        unassignedTaskCount: 0,
        averageCycleTimeHours: 4,
        onTimeCompletionPercent: 100,
        health: "on_track",
      },
    ],
  };
}

describe("weekly priority task selection", () => {
  it("keeps only open tasks from projects that need attention", () => {
    const selected = selectWeeklyPriorityTasks(
      [report()],
      [
        task("attention-open", "project-attention"),
        task("attention-completed", "project-attention", "completed"),
        task("on-track-open", "project-on-track"),
        task("unlinked-open", null),
      ],
    );

    expect(selected.map((item) => item.id)).toEqual(["attention-open"]);
  });

  it("limits the mobile priority summary to three tasks", () => {
    const selected = selectWeeklyPriorityTasks(
      [report()],
      Array.from({ length: 5 }, (_, index) =>
        task(`attention-${index + 1}`, "project-attention"),
      ),
    );

    expect(selected).toHaveLength(3);
  });
});
