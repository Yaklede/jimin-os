import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  type GoogleChatAccount,
  type ProjectGoogleChatSource,
  type ProjectInflowItem,
} from "../api/googleChat";
import { copy } from "../copy";
import {
  ProjectInflowPanel,
  isExistingTaskFollowUp,
  projectInflowAttentionCount,
  localInputToIso,
  resolvePromotionDeadline,
} from "./ProjectInflowPanel";

describe("project inflow deadline", () => {
  it("keeps a selected local deadline when promoting a Chat request", () => {
    const input = "2026-07-24T18:30";

    expect(localInputToIso(input)).toBe(new Date(input).toISOString());
  });

  it("does not turn an invalid deadline into an empty value", () => {
    expect(localInputToIso("not-a-date")).toBeUndefined();
    expect(localInputToIso("")).toBeUndefined();
  });

  it("requires a deadline unless the user explicitly chooses otherwise", () => {
    expect(resolvePromotionDeadline("", false)).toBeUndefined();
    expect(resolvePromotionDeadline("not-a-date", false)).toBeUndefined();
    expect(resolvePromotionDeadline("", true)).toEqual({
      dueAt: null,
      withoutDeadline: true,
    });
  });

  it("keeps the native date input value in the promotion request", () => {
    const input = "2026-07-29T18:30";

    expect(resolvePromotionDeadline(input, false)).toEqual({
      dueAt: new Date(input).toISOString(),
      withoutDeadline: false,
    });
  });
});

describe("project inflow attention", () => {
  it("counts only pending conversations as attention items", () => {
    expect(
      projectInflowAttentionCount([
        inflowItem("new-request"),
        inflowItem("follow-up", { promotedTaskId: "task-1" }),
        inflowItem("handled", { status: "promoted" }),
        inflowItem("dismissed", { status: "dismissed" }),
      ]),
    ).toBe(2);
  });

  it("treats a pending reply linked to a task as a follow-up", () => {
    const item = inflowItem("follow-up", { promotedTaskId: "task-1" });

    expect(isExistingTaskFollowUp(item)).toBe(true);
    expect(
      renderPanel({
        items: [item],
      }),
    ).toContain(copy.projects.inflowFollowUpTitle);
    expect(renderPanel({ items: [item] })).toContain(
      copy.projects.inflowFollowUpOpenTask,
    );
    expect(renderPanel({ items: [item] })).toContain(
      copy.projects.inflowFollowUpDone,
    );
    expect(renderPanel({ items: [item] })).not.toContain(
      `>${copy.projects.inflowPromote}</button>`,
    );
  });

  it("keeps handled conversations inside a collapsed history disclosure", () => {
    const markup = renderPanel({
      items: [inflowItem("handled", { status: "promoted" })],
    });

    expect(markup).toContain('class="project-inflow__history"');
    expect(markup).not.toContain('class="project-inflow__history" open=""');
    expect(markup).toContain(copy.projects.inflowRecentTitle);
    expect(markup).toContain(copy.projects.inflowEmpty);
  });

  it("shows account recovery instead of a generic load error", () => {
    const markup = renderPanel({
      accounts: [googleChatAccount("reauth_required")],
      problemMessage: copy.projects.inflowLoadProblem,
    });

    expect(markup).toContain(copy.projects.inflowReconnectProblem);
    expect(markup).toContain(copy.projects.inflowReconnectAction);
    expect(markup).not.toContain(copy.projects.inflowLoadProblem);
  });
});

function renderPanel(
  overrides: Partial<ComponentProps<typeof ProjectInflowPanel>> = {},
): string {
  const props: ComponentProps<typeof ProjectInflowPanel> = {
    accountsAvailable: true,
    accounts: [googleChatAccount("active")],
    spaces: [],
    sources: [googleChatSource()],
    items: [],
    loading: false,
    saving: false,
    problemMessage: undefined,
    onConnectAccount: async () => undefined,
    onLoadSpaces: async () => undefined,
    onCreateSource: async () => undefined,
    onDeleteSource: async () => undefined,
    onSyncSource: async () => undefined,
    onPromote: async () => undefined,
    onDismiss: async () => undefined,
    onRetryAnalysis: async () => undefined,
    onRetryCompletion: async () => undefined,
    onOpenTask: () => undefined,
    ...overrides,
  };
  return renderToStaticMarkup(createElement(ProjectInflowPanel, props));
}

function googleChatAccount(
  status: GoogleChatAccount["status"],
): GoogleChatAccount {
  return {
    id: "account-1",
    email: "company@example.com",
    status,
    lastSuccessfulSyncAt: null,
    lastErrorCode: null,
    reauthRequired: status === "reauth_required",
    version: 1,
  };
}

function googleChatSource(): ProjectGoogleChatSource {
  return {
    id: "source-1",
    projectId: "project-1",
    accountId: "account-1",
    accountEmail: "company@example.com",
    spaceName: "spaces/1",
    displayName: "업무 공간",
    enabled: true,
    acknowledgeWithReaction: true,
    lastSuccessfulSyncAt: null,
    lastErrorCode: null,
    version: 1,
  };
}

function inflowItem(
  id: string,
  overrides: Partial<ProjectInflowItem> = {},
): ProjectInflowItem {
  const receivedAt = "2026-07-30T08:00:00.000Z";
  return {
    id,
    projectId: "project-1",
    projectName: "프로젝트",
    sourceId: "source-1",
    sourceName: "업무 공간",
    senderName: "담당자",
    sentByOwner: false,
    contentText: "확인 부탁드립니다.",
    suggestedTaskTitle: "요청 내용 확인",
    suggestedTaskNotes: "요청 내용을 확인하고 결과를 공유합니다.",
    referenceLinks: [],
    suggestedAssigneeName: null,
    suggestedDueAt: "2026-07-31T09:00:00.000Z",
    suggestedPriority: 1,
    analysisStatus: "ready",
    analysisClassification: "new_task",
    analysisConfidence: 90,
    analysisSummary: "확인이 필요한 업무 요청입니다.",
    analysisErrorCode: null,
    messageCount: 1,
    firstReceivedAt: receivedAt,
    receivedAt,
    messages: [
      {
        senderName: "담당자",
        sentByOwner: false,
        contentText: "확인 부탁드립니다.",
        receivedAt,
      },
    ],
    status: "pending",
    promotedTaskId: null,
    acknowledged: true,
    completionStatus: "not_requested",
    completionReactionCompleted: false,
    completionReplyCompleted: false,
    completionErrorCode: null,
    completionAttemptCount: 0,
    assigneeOptions: [],
    notifiableAssigneeNames: [],
    assigneeNotificationAvailable: false,
    version: 1,
    ...overrides,
  };
}
