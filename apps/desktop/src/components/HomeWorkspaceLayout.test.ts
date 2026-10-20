import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import type { Task } from "../api/planning";
import { designPreviewInflow } from "../design-preview-inflow";
import { HomeWorkspace } from "./HomeWorkspace";

const layout = vi.hoisted(() => ({ mobile: false }));
vi.mock("../useMobileLayout", () => ({
  useMobileLayout: () => layout.mobile,
}));

function homeProps(): ComponentProps<typeof HomeWorkspace> {
  const task: Task = {
    id: "layout-task",
    projectId: null,
    title: "화면 순서 확인",
    notes: null,
    status: "open",
    priority: 2,
    dueAt: new Date().toISOString(),
    completedAt: null,
    version: 1,
  };
  const noop = () => {};
  const done = async () => {};
  const loadTask = async () => task;
  return {
    scheduledWork: createElement("div", {
      "data-testid": "scheduled-layout-test",
    }),
    snapshot: {
      schedule: [],
      tasks: [task],
      dueTasks: [],
      inflow: [designPreviewInflow()[0]],
      recentInflow: [
        {
          ...designPreviewInflow()[0],
          status: "promoted",
          promotedTaskId: task.id,
          completionStatus: "failed",
        },
      ],
      deliveryRecovery: [
        {
          id: "delivery-failed",
          kind: "webhook",
          projectId: null,
          title: "알림 실패 예시",
          status: "failed",
          canRetry: true,
          attemptCount: 3,
          createdAt: new Date().toISOString(),
        },
      ],
      recommendations: [],
      weeklyReports: [],
    },
    loading: false,
    error: undefined,
    assistantReady: true,
    assistantConversationId: undefined,
    assistantRequest: undefined,
    assistantJob: undefined,
    assistantMessage: undefined,
    onLoadAllTasks: async () => [task],
    onOpenAssistant: noop,
    onOpenPlanning: noop,
    onStartNewAssistant: async () => true,
    onSendAssistant: async () => true,
    onCompleteTask: done,
    onLoadAssistantTask: loadTask,
    onCompleteAssistantTask: loadTask,
    onRestoreAssistantTask: loadTask,
    onEditAssistantTask: noop,
    onEditAssistantSchedule: noop,
    onEditTask: noop,
    onEditSchedule: noop,
    onOpenPlanningTask: noop,
    onOpenTask: noop,
    onOpenProject: noop,
    onOpenSchedule: noop,
    onOpenDecisionInbox: noop,
    onOpenMeetings: noop,
    onOpenSettings: noop,
    onDecideRecommendation: async () => true,
    inflowSaving: false,
    onPromoteInflow: done,
    onDismissInflow: done,
    onRetryInflowAnalysis: done,
    onRetryInflowCompletion: done,
    gmailInflowItems: [],
    gmailInflowProjects: [],
    gmailInflowLoading: false,
    gmailInflowLoadingMore: false,
    gmailInflowLoadMoreError: false,
    gmailInflowHasMore: false,
    gmailInflowError: undefined,
    gmailInflowSavingId: undefined,
    onReloadGmailInflow: noop,
    onLoadMoreGmailInflow: noop,
    onPromoteGmailInflow: done,
    onDismissGmailInflow: done,
    onDeferGmailInflow: done,
    onRetryGmailInflowAnalysis: done,
  };
}

describe("home request-first layout", () => {
  it.each([false, true])(
    "keeps incoming and assistant requests before direct tasks (mobile=%s)",
    (mobile) => {
      layout.mobile = mobile;
      const markup = renderToStaticMarkup(
        createElement(HomeWorkspace, homeProps()),
      );
      const incoming = markup.indexOf('class="inflow-review-queues"');
      const assistant = markup.indexOf('class="home-command"');
      const tasks = markup.indexOf(
        'class="mobile-task-queue home-direct-tasks"',
      );
      const scheduled = markup.indexOf('data-testid="scheduled-layout-test"');
      expect(scheduled).toBeGreaterThan(-1);
      expect(incoming).toBeGreaterThan(scheduled);
      expect(assistant).toBeGreaterThan(incoming);
      expect(tasks).toBeGreaterThan(assistant);
      expect(markup).not.toContain('class="inflow-recovery"');
      expect(
        markup.match(/class="mobile-task-queue home-direct-tasks"/g),
      ).toHaveLength(1);
      expect(markup).toContain("담당자별");
      expect(markup).toContain("일자별");
      expect(markup).toContain("화면 순서 확인");
      expect(markup.match(/data-testid="scheduled-layout-test"/g)).toHaveLength(
        1,
      );
    },
  );
});
