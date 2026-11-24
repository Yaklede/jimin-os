import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import {
  hasNewGoogleChatAuthorization,
  type GoogleChatAccount,
  type ProjectGoogleChatSource,
  type ProjectInflowItem,
} from "../api/googleChat";
import { copy } from "../copy";
import {
  InflowItemRow,
  ProjectInflowPanel,
  isExistingTaskFollowUp,
  inflowContextScrollBehavior,
  inflowConversationKey,
  mergeInflowDraftValues,
  nextInflowDraftBaseRevision,
  projectInflowPromotionProblem,
  projectInflowAttentionCount,
  localInputToIso,
  resolvePromotionDeadline,
  inflowRegistrationSummary,
} from "./ProjectInflowPanel";

describe("project inflow deadline", () => {
  it("exposes assignment immediately for home without changing the existing follow-up actions", () => {
    const row = (overrides: Partial<ProjectInflowItem>) =>
      renderToStaticMarkup(
        createElement(InflowItemRow, {
          item: inflowItem("inline", overrides),
          inlinePromotion: true,
          saving: false,
          onPromote: async () => undefined,
          onDismiss: async () => undefined,
          onRetryAnalysis: async () => undefined,
          onOpenTask: () => undefined,
        }),
      );
    const ready = row({
      suggestedAssigneeName: "김경주",
      assigneeOptions: ["김경주"],
      notifiableAssigneeNames: ["김경주"],
    });
    expect(ready).toContain('data-inline="true"');
    expect(ready).toContain(copy.projects.inflowInlineAssignment);
    expect(ready).toContain(copy.projects.inflowAssignmentNotify("김경주"));
    expect(ready).toContain('class="inline-assignment__details"');
    expect(ready).not.toContain("<dialog");
    expect(row({ promotedTaskId: "already-open" })).toContain(
      copy.projects.inflowFollowUpTitle,
    );
    expect(row({ promotedTaskId: "already-open" })).not.toContain(
      'data-inline="true"',
    );
    expect(row({ analysisStatus: "failed" })).not.toContain(
      'data-inline="true"',
    );
  });
  it("summarizes the same Seoul deadline and notification availability used to register", () => {
    expect(
      inflowRegistrationSummary(
        "김경주",
        ["김경주"],
        resolvePromotionDeadline("2026-11-20T15:00", false),
      ),
    ).toEqual({
      assigneeLabel: "김경주",
      deadlineLabel: "11월 20일 오후 3:00",
      chatLabel: copy.projects.inflowSummaryChatScheduled,
      canNotifyAssignee: true,
    });
    expect(inflowRegistrationSummary("", ["김경주"], undefined)).toEqual({
      assigneeLabel: copy.projects.inflowSummaryNoAssignee,
      deadlineLabel: copy.projects.inflowSummaryChooseDeadline,
      chatLabel: copy.projects.inflowSummaryChatOff,
      canNotifyAssignee: false,
    });
    expect(
      inflowRegistrationSummary(
        "송천안",
        ["김경주"],
        resolvePromotionDeadline("", true),
      ),
    ).toMatchObject({
      deadlineLabel: copy.projects.inflowSummaryNoDeadline,
      canNotifyAssignee: false,
    });
  });

  it("shows a review action for unread requests but not for already reviewed ones", () => {
    const unread = renderPanel({ items: [inflowItem("unread")] });
    const reviewed = renderPanel({
      items: [inflowItem("reviewed", { reviewed: true })],
    });

    expect(unread).toContain(`${copy.projects.inflowMarkSeen}</button>`);
    expect(reviewed).not.toContain(`${copy.projects.inflowMarkSeen}</button>`);
    expect(reviewed).toContain(copy.projects.inflowReviewed);
  });

  it("offers independent collection and delivery purposes without treating a reporting room as acknowledged inflow", () => {
    const markup = renderPanel({
      sources: [{ ...googleChatSource(), purpose: "delivery" }],
      onUpdateSourcePurpose: async () => undefined,
    });
    expect(markup).toContain("업무 수집용");
    expect(markup).toContain("배정·완료 보고용");
    expect(markup).toContain("역할 저장");
    expect(markup).not.toContain('class="project-inflow__ack-state"');
  });
  it("restores a saved draft without automatically opening the registration modal", () => {
    vi.stubGlobal("window", {
      sessionStorage: {
        getItem: () =>
          JSON.stringify({
            savedAt: Date.now(),
            baseRevision: 1,
            dirtyFields: ["title", "assigneeName", "dueAt"],
            title: "작성 중인 제목",
            notes: "작성 중인 설명",
            assigneeName: "김경주",
            priority: "2",
            dueAt: "2026-10-06T23:45",
            withoutDeadline: false,
          }),
        removeItem() {},
      },
    });
    try {
      const markup = renderPanel({ items: [inflowItem("saved-draft")] });
      expect(markup).toContain(copy.projects.inflowPromote);
      expect(markup).not.toContain("<dialog");
      expect(markup).not.toContain("<textarea");
    } finally {
      vi.unstubAllGlobals();
    }
  });
  it("keeps a selected local deadline when promoting a Chat request", () => {
    const input = "2026-07-24T18:30";

    expect(localInputToIso(input)).toBe("2026-07-24T09:30:00.000Z");
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
      dueAt: "2026-07-29T09:30:00.000Z",
      withoutDeadline: false,
    });
  });

  it("keeps UI identity on the stable conversation id", () => {
    expect(
      inflowConversationKey(
        inflowItem("latest-message", { conversationId: "conversation-1" }),
      ),
    ).toBe("conversation-1");
    expect(
      inflowConversationKey(
        inflowItem("mutation-snapshot", { conversationId: null }),
      ),
    ).toBe("mutation-snapshot");
  });

  it("keeps an archived task separate from new work in the same conversation", () => {
    const active = inflowItem("new", { conversationId: "thread" });
    const archived = inflowItem("old", {
      conversationId: "thread",
      status: "promoted",
      promotedTaskId: "closed-task",
    });
    expect(inflowConversationKey(active)).not.toBe(
      inflowConversationKey(archived),
    );
  });

  it("updates AI suggestions without overwriting fields the user changed", () => {
    const current = {
      title: "사용자가 다듬은 제목",
      notes: "이전 정리",
      assigneeName: "김경주",
      priority: "1",
      dueAt: "2026-07-31T18:00",
      withoutDeadline: false,
    };

    expect(
      mergeInflowDraftValues(
        current,
        {
          title: "새 답글을 반영한 제목",
          notes: "새 답글을 반영한 상세 내용",
          assigneeName: "주홍석",
          priority: "2",
          dueAt: "2026-08-01T18:00",
          withoutDeadline: false,
        },
        ["title", "assigneeName"],
      ),
    ).toEqual({
      title: "사용자가 다듬은 제목",
      notes: "새 답글을 반영한 상세 내용",
      assigneeName: "김경주",
      priority: "2",
      dueAt: "2026-08-01T18:00",
      withoutDeadline: false,
    });
  });

  it("keeps the draft revision behind while the user is editing", () => {
    expect(nextInflowDraftBaseRevision(2, 3, true)).toBe(2);
    expect(nextInflowDraftBaseRevision(2, 3, false)).toBe(3);
    expect(nextInflowDraftBaseRevision(3, null, false)).toBe(3);
  });

  it("uses an immediate scroll when reduced motion is preferred", () => {
    expect(inflowContextScrollBehavior(true)).toBe("auto");
    expect(inflowContextScrollBehavior(false)).toBe("smooth");
  });

  it("blocks stale analysis with guidance before a promotion can start", () => {
    const markup = renderPanel({
      items: [
        inflowItem("stale", {
          analysisStatus: "stale",
          sourceRevision: 3,
          analyzedRevision: 2,
        }),
      ],
    });

    expect(markup).toContain(copy.projects.inflowPromotionStale);
    expect(markup).toContain(copy.projects.inflowAnalysisRetry);
    expect(markup).not.toContain(copy.projects.inflowPromote);
    expect(projectInflowPromotionProblem("missing_context")).toBe(
      copy.projects.inflowPromotionContextMissing,
    );
  });

  it("keeps the previous summary without offering a duplicate retry during refresh", () => {
    const markup = renderPanel({
      items: [
        inflowItem("refresh", {
          analysisStatus: "refreshing",
          analysisSummary: "기존 분석 결과",
          sourceRevision: 3,
          analyzedRevision: 2,
        }),
      ],
    });
    expect(markup).toContain("기존 분석 결과");
    expect(markup).not.toContain(copy.projects.inflowAnalysisRetry);
  });

  it("keeps fetched reference evidence collapsed and read-only", () => {
    const originalContent = "ITSM 요청 원문은 편집할 수 없어야 합니다.";
    const markup = renderPanel({
      items: [
        inflowItem("evidence", {
          referenceDocuments: [
            {
              provider: "itsm",
              url: "https://itsm.example/issues/3876",
              externalId: "3876",
              title: "정산 방식 표기 요청",
              originalContent,
              errorCode: null,
            },
          ],
        }),
      ],
    });

    expect(markup).toContain('class="project-inflow-item__evidence"');
    expect(markup).not.toContain(
      'class="project-inflow-item__evidence" open=""',
    );
    expect(markup).toContain(copy.projects.inflowEvidenceSummary(1));
    expect(markup).toContain("정산 방식 표기 요청");
    expect(markup).toContain(originalContent);
    expect(markup).not.toContain(`<textarea>${originalContent}</textarea>`);
  });
});

describe("project inflow attention", () => {
  it("keeps reviewed pending requests in a collapsed section without counting them as unread", () => {
    const item = inflowItem("reviewed", { reviewed: true });
    expect(projectInflowAttentionCount([item])).toBe(0);
    const markup = renderPanel({ items: [item] });
    expect(markup).toContain(copy.projects.inflowReviewedTitle);
    expect(markup).toContain(copy.projects.inflowPromote);
    expect(markup).toContain('class="project-inflow__history"');
    expect(markup).not.toContain('class="project-inflow__history" open=""');
  });
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

  it("offers cleanup for a known-unused account only and keeps recovery visible", () => {
    const company = {
      ...googleChatAccount("reauth_required"),
      connectedSpaceCount: 2,
    };
    const unused = {
      ...googleChatAccount("active"),
      id: "unused",
      email: "unused@example.test",
      connectedSpaceCount: 0,
    };
    const elsewhere = {
      ...unused,
      id: "elsewhere",
      email: "elsewhere@example.test",
      connectedSpaceCount: 1,
    };
    const unknown = {
      ...unused,
      id: "unknown",
      email: "unknown@example.test",
      connectedSpaceCount: undefined,
    };
    const markup = renderPanel({
      accounts: [unused, company, elsewhere, unknown],
      onDisconnectAccount: async () => undefined,
    });
    expect(markup).toContain('aria-label="unused@example.test 계정 해제"');
    expect(markup).not.toMatch(
      /aria-label="(?:company@example.com|elsewhere@example.test|unknown@example.test) 계정 해제"/,
    );
    expect(markup).toContain(
      "company@example.com 계정으로 다시 연결해 주세요.",
    );
    expect(markup).toContain(copy.projects.inflowReconnectAction);
    expect(markup).toContain('<option value="" disabled="" selected="">');
  });

  it("prefers the active account used by the current project over list order", () => {
    const company = googleChatAccount("active");
    const personal = {
      ...company,
      id: "personal",
      email: "wrong@example.test",
    };
    const markup = renderPanel({ accounts: [personal, company] });
    expect(markup).toContain('<option value="account-1" selected="">');
  });

  it("does not finish a reconnect poll just because another account was already active", () => {
    const company = googleChatAccount("reauth_required");
    const unrelated = {
      ...company,
      id: "unrelated",
      status: "active" as const,
    };
    const versions = new Map([
      [company.id, company.version],
      [unrelated.id, unrelated.version],
    ]);
    expect(hasNewGoogleChatAuthorization([company, unrelated], versions)).toBe(
      false,
    );
    expect(
      hasNewGoogleChatAuthorization(
        [{ ...company, status: "active", version: 2 }, unrelated],
        versions,
      ),
    ).toBe(true);
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
    conversationId: `conversation-${id}`,
    representativeItemId: `representative-${id}`,
    sourceRevision: 1,
    analyzedRevision: 1,
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
    referenceDocuments: [],
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
