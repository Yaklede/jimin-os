import { describe, expect, it } from "vitest";
import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { type Task } from "../api/planning";
import { type WeeklyReport } from "../api/projects";
import {
  HomeAssistantCommand,
  selectWeeklyPriorityTasks,
} from "./HomeWorkspace";

describe("home new request action", () => {
  const base: ComponentProps<typeof HomeAssistantCommand> = {
    ready: true,
    conversationId: "home-conversation",
    request: "할 일 확인해 줘",
    job: undefined,
    message: undefined,
    focused: false,
    onFocusChange() {},
    onOpenAssistant() {},
    onStartNew: async () => true,
    onSend: async () => true,
    onLoadTask: async () => task("task", null),
    onCompleteTask: async () => task("task", null),
    onRestoreTask: async () => task("task", null),
    onEditTask() {},
    onEditSchedule() {},
    onOpenTask() {},
    onOpenProject() {},
    onOpenSchedule() {},
  };
  it("renders a visible new request action even when the previous result is collapsed", () => {
    const markup = renderToStaticMarkup(
      createElement(HomeAssistantCommand, base),
    );
    expect(markup).toContain("새 요청 시작하기");
    expect(markup).toContain("home-command__new-request");
    expect(markup).toContain("이어서 요청하기");
  });
  it.each(["completed", "failed", "cancelled", "declined"] as const)(
    "enables the new request action after %s",
    (state) => {
      const markup = renderToStaticMarkup(
        createElement(HomeAssistantCommand, {
          ...base,
          job: {
            id: "job",
            conversationId: "home-conversation",
            state,
            createdAt: "2026-10-06T00:00:00Z",
            finishedAt: "2026-10-06T00:01:00Z",
            pendingAction: null,
            version: 1,
          },
        }),
      );
      expect(
        markup.match(/<button[^>]*home-command__new-request[^>]*>/)?.[0],
      ).not.toContain("disabled");
    },
  );
  it.each(["running", "queued", "waiting_approval", "retry_wait"] as const)(
    "keeps the button visible but disabled during %s",
    (state) => {
      const markup = renderToStaticMarkup(
        createElement(HomeAssistantCommand, {
          ...base,
          job: {
            id: "job",
            conversationId: "home-conversation",
            state,
            createdAt: "2026-10-06T00:00:00Z",
            finishedAt: null,
            pendingAction: null,
            version: 1,
          },
        }),
      );
      expect(markup).toContain("새 요청 시작하기");
      expect(
        markup.match(/<button[^>]*home-command__new-request[^>]*>/)?.[0],
      ).toContain("disabled");
    },
  );
});

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
