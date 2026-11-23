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
    homeChatDraft: { text: "" },
    setHomeChatDraft: noop,
    onOpenConversationHistory: noop,
    deliveryFailureCount: 2,
    onOpenDeliveryHistory: noop,
    onLoadAllTasks: async () => [task],
    onOpenAssistant: noop,
    onOpenPlanning: noop,
    onSendAssistant: async () => true,
    onCompleteTask: done,
    onLoadAssistantTask: loadTask,
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
  it("starts with compact filters and counts a shared task once across assignee groups", () => {
    const props = homeProps();
    props.snapshot!.tasks[0].assigneeName = "김경주, 송천안";
    const markup = renderToStaticMarkup(createElement(HomeWorkspace, props));
    expect(markup).toContain(
      'aria-expanded="false" aria-controls="home-task-filter-controls"',
    );
    expect(markup).toContain('id="home-task-filter-controls" hidden=""');
    expect(markup.match(/class="home-direct-tasks__shared"/g)).toHaveLength(2);
    expect(markup).toContain(
      'id="mobile-task-queue-title">할 일 <small>1개</small>',
    );
  });
  it.each([false, true])(
    "keeps scheduling and today's shortcut above the queue, then direct tasks (mobile=%s)",
    (mobile) => {
      layout.mobile = mobile;
      const markup = renderToStaticMarkup(
        createElement(HomeWorkspace, homeProps()),
      );
      const incoming = markup.indexOf('class="inflow-review-queues"');
      const assistant = markup.indexOf(
        'class="home-command home-command--compact"',
      );
      const tasks = markup.indexOf(
        'class="mobile-task-queue home-direct-tasks"',
      );
      const scheduled = markup.indexOf('data-testid="scheduled-layout-test"');
      const today = markup.indexOf('class="home-today-shortcut');
      expect(scheduled).toBeGreaterThan(-1);
      expect(today).toBeGreaterThan(scheduled);
      expect(incoming).toBeGreaterThan(today);
      expect(tasks).toBeGreaterThan(incoming);
      expect(assistant).toBeGreaterThan(tasks);
      expect(markup).toContain('aria-controls="home-today-task-list"');
      expect(markup).not.toContain('class="inflow-recovery"');
      expect(markup).toContain("반영 실패 2건 확인하기");
      expect(markup).not.toContain("Chat 반영 확인하기");
      expect(markup).not.toContain("이 알림만 다시 보내기");
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
  it("does not leave an empty delivery row and keeps the completion-review tab", () => {
    const markup = renderToStaticMarkup(
      createElement(HomeWorkspace, {
        ...homeProps(),
        completionReviews: createElement("p", null, "완료 보고 검수용"),
        deliveryFailureCount: 0,
      }),
    );
    expect(markup).not.toContain("home-delivery-attention");
    expect(markup).toContain('id="home-inflow-tab-completion"');
    expect(markup).toContain("완료 보고 검수용");
    expect(markup).toContain('aria-controls="home-today-task-list"');
  });
});
