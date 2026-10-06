import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { type GmailInflowCandidate } from "../api/gmailInflow";
import { type ProjectInflowItem } from "../api/googleChat";
import { type Recommendation } from "../api/home";
import { copy } from "../copy";
import {
  DecisionInboxWorkspace,
  inflowDecisionSummary,
  isConversationDecision,
  isDecisionActionableNow,
  isDecisionInProgress,
  isProjectInflowDecisionItem,
} from "./DecisionInboxWorkspace";

function recommendation(
  suggestedActionKind: Recommendation["suggestedActionKind"],
  suggestedEntityId: string | null,
): Recommendation {
  return {
    id: "019f68cb-9400-7000-8000-000000000021",
    workspaceId: null,
    projectId: null,
    goalId: null,
    signalId: "019f68cb-9400-7000-8000-000000000022",
    title: "일정 시간을 다시 정해 주세요",
    rationale: "기존 일정과 시간이 겹쳐요.",
    expectedEffect: "두 일정을 모두 준비할 수 있어요.",
    riskSummary: "이동 시간을 함께 확인해 주세요.",
    confidence: 100,
    urgency: 2,
    impact: 2,
    riskLevel: 1,
    effortMinutes: 5,
    suggestedActionKind,
    suggestedEntityId,
    status: "pending",
    validUntil: "2026-07-28T00:00:00Z",
    revisitAt: null,
    createdAt: "2026-07-27T00:00:00Z",
    updatedAt: "2026-07-27T00:00:00Z",
    version: 1,
  };
}

function promotableInflow(): ProjectInflowItem {
  return {
    id: "019f68cb-9400-7000-8000-000000000030",
    projectId: "019f68cb-9400-7000-8000-000000000034",
    projectName: "비스킷링크",
    sourceId: "019f68cb-9400-7000-8000-000000000035",
    sourceName: "Google Chat",
    contentText: "신규 업무를 확인해 주세요.",
    receivedAt: "2026-07-31T01:00:00Z",
    suggestedTaskTitle: "신규 업무 확인",
    suggestedTaskNotes: "요청 내용을 확인하고 결과를 공유합니다.",
    suggestedPriority: 1,
    suggestedAssigneeName: null,
    suggestedDueAt: null,
    status: "pending",
    promotedTaskId: null,
    analysisStatus: "ready",
    analysisClassification: "new_task",
    conversationId: "019f68cb-9400-7000-8000-000000000031",
    representativeItemId: "019f68cb-9400-7000-8000-000000000032",
    sourceRevision: 2,
    analyzedRevision: 2,
    version: 1,
  } as ProjectInflowItem;
}

function gmailCandidate(): GmailInflowCandidate {
  return {
    id: "gmail-candidate",
    accountId: "account-company",
    accountEmail: "work@company.example",
    workspaceId: "workspace-company",
    workspaceName: "회사",
    workspaceScope: "company",
    messageId: "message",
    providerMessageId: "provider-message",
    providerThreadId: "thread",
    senderName: "고객 지원팀",
    senderEmail: "support@example.com",
    subject: "계약서 확인 요청",
    snippet: "계약서를 확인해 주세요.",
    bodyText: "계약서 원문을 확인하고 결과를 알려 주세요.",
    originalThreadUrl: "https://mail.google.com/mail/u/0/#inbox/thread",
    referenceLinks: ["https://docs.example.com/contract"],
    receivedAt: "2026-07-31T01:30:00Z",
    suggestedTaskTitle: "계약서 검토",
    suggestedTaskNotes: "계약서를 검토하고 결과를 공유합니다.",
    suggestedAssigneeName: null,
    suggestedPriority: 2,
    suggestedDueAt: null,
    analysisStatus: "ready",
    analysisClassification: "new_task",
    analysisConfidence: 94,
    analysisSummary: "검토가 필요한 업무 요청입니다.",
    analysisErrorCode: null,
    status: "pending",
    promotedTaskId: null,
    deferredUntil: null,
    version: 1,
  };
}

describe("decision inbox actions", () => {
  it("surfaces a discovered ITSM project as an explicit owner decision", () => {
    const markup = renderToStaticMarkup(
      createElement(DecisionInboxWorkspace, {
        recommendations: [],
        inflowItems: [],
        itsmCandidates: [
          {
            projectName: "비스킷링크",
            connection: {
              id: "019f68cb-9400-7000-8000-000000000041",
              projectId: "019f68cb-9400-7000-8000-000000000042",
              enabled: true,
              confirmationStatus: "confirmation_required",
              candidateProjectName: "비스킷링크 ITSM",
              version: 2,
            },
          },
        ],
        loading: false,
        error: undefined,
        inflowSaving: false,
        gmailReview: emptyGmailReview(),
        onOpenConversation: () => undefined,
        onOpenTask: async () => undefined,
        onPromoteInflow: async () => undefined,
        onDismissInflow: async () => undefined,
        onRetryInflowAnalysis: async () => undefined,
        onRetryInflowCompletion: async () => undefined,
        onConfirmItsm: async () => undefined,
        onDecide: async () => true,
        onRetryAnalysis: async () => true,
      }),
    );

    expect(markup).toContain(copy.decisions.itsmTitle);
    expect(markup).toContain("비스킷링크 ITSM");
    expect(markup).toContain(copy.decisions.confirmItsm);
  });

  it("routes schedule conflict decisions back to their conversation", () => {
    expect(
      isConversationDecision(
        recommendation(
          "request_analysis",
          "019f68cb-9400-7000-8000-000000000023",
        ),
      ),
    ).toBe(true);
  });

  it("keeps deferred decisions out of the current list until revisit time", () => {
    const deferred = {
      ...recommendation(
        "request_analysis",
        "019f68cb-9400-7000-8000-000000000023",
      ),
      status: "deferred" as const,
      revisitAt: "2026-07-27T08:00:00Z",
    };

    expect(
      isDecisionActionableNow(
        deferred,
        new Date("2026-07-27T07:59:59Z").getTime(),
      ),
    ).toBe(false);
    expect(
      isDecisionActionableNow(
        deferred,
        new Date("2026-07-27T08:00:00Z").getTime(),
      ),
    ).toBe(true);
  });

  it("does not treat informational review cards as conversation decisions", () => {
    expect(
      isConversationDecision(
        recommendation("review", "019f68cb-9400-7000-8000-000000000023"),
      ),
    ).toBe(false);
    expect(
      isConversationDecision(recommendation("request_analysis", null)),
    ).toBe(false);
  });

  it("summarizes the choices still missing from a new work item", () => {
    const item = {
      suggestedAssigneeName: null,
      suggestedDueAt: null,
    } as ProjectInflowItem;

    expect(inflowDecisionSummary(item)).toBe("업무로 등록할지 · 담당자 · 마감");
    expect(
      inflowDecisionSummary({
        ...item,
        suggestedAssigneeName: "김경주",
        suggestedDueAt: "2026-08-01T09:00:00Z",
      }),
    ).toBe("업무로 등록할지");
  });

  it("keeps every pending Chat review and failed completion reachable", () => {
    const item = promotableInflow();

    expect(isProjectInflowDecisionItem(item)).toBe(true);
    expect(
      isProjectInflowDecisionItem({
        ...item,
        analysisStatus: "refreshing",
      }),
    ).toBe(false);
    expect(
      isProjectInflowDecisionItem({
        ...item,
        analysisClassification: "question",
      }),
    ).toBe(false);
    expect(
      isProjectInflowDecisionItem({
        ...item,
        analysisStatus: "failed",
      }),
    ).toBe(true);
    expect(
      isProjectInflowDecisionItem({
        ...item,
        analysisStatus: "stale",
      }),
    ).toBe(true);
    expect(
      isProjectInflowDecisionItem({
        ...item,
        promotedTaskId: "019f68cb-9400-7000-8000-000000000033",
      }),
    ).toBe(true);
    expect(
      isProjectInflowDecisionItem({
        ...item,
        status: "promoted",
        completionStatus: "failed",
      }),
    ).toBe(true);
    expect(
      isProjectInflowDecisionItem({
        ...item,
        status: "promoted",
        completionStatus: "sent",
      }),
    ).toBe(false);
  });

  it("lets the owner organize a Chat request without leaving the decision inbox", () => {
    const markup = renderDecisionInbox({ inflowItems: [promotableInflow()] });

    expect(markup).toContain(copy.projects.inflowPromote);
    expect(markup).toContain(copy.projects.inflowDismiss);
    expect(markup).not.toContain(copy.decisions.openInProject);
  });

  it("opens the existing task when a linked Chat thread receives a follow-up", () => {
    const markup = renderDecisionInbox({
      inflowItems: [
        {
          ...promotableInflow(),
          promotedTaskId: "019f68cb-9400-7000-8000-000000000033",
        },
      ],
    });

    expect(markup).toContain(copy.projects.inflowFollowUpTitle);
    expect(markup).toContain(copy.projects.inflowFollowUpOpenTask);
  });

  it("shows Gmail decisions with their inline fields and source", () => {
    const markup = renderDecisionInbox({
      gmailReview: {
        ...emptyGmailReview(),
        items: [gmailCandidate()],
      },
    });

    expect(markup).toContain("계약서 확인 요청");
    expect(markup).toContain(copy.gmailInflow.openOriginal);
    expect(markup).toContain(copy.gmailInflow.promote);
  });

  it("keeps completed decisions collapsed without hiding approved or executing work", () => {
    expect(
      isDecisionInProgress({
        ...recommendation("review", null),
        status: "approved",
      }),
    ).toBe(true);
    expect(
      isDecisionInProgress({
        ...recommendation("review", null),
        status: "executing",
      }),
    ).toBe(true);
    expect(
      isDecisionInProgress({
        ...recommendation("review", null),
        status: "executed",
      }),
    ).toBe(false);

    const markup = renderDecisionInbox({
      recommendations: [
        { ...recommendation("review", null), status: "executed" },
      ],
    });
    expect(markup).toContain('class="decision-history"');
    expect(markup).not.toContain('<details class="decision-history" open="">');
  });

  it("keeps failed decisions visible with a direct analysis retry", () => {
    const markup = renderDecisionInbox({
      recommendations: [
        { ...recommendation("review", null), status: "failed" },
      ],
    });

    expect(markup).toContain(copy.decisions.retryTitle);
    expect(markup).toContain(copy.decisions.retryAnalysis);
    expect(markup).not.toContain('class="decision-history"');
  });
});

function emptyGmailReview(): ComponentProps<
  typeof DecisionInboxWorkspace
>["gmailReview"] {
  return {
    items: [],
    projects: [],
    loading: false,
    loadingMore: false,
    loadMoreError: false,
    hasMore: false,
    error: undefined,
    savingId: undefined,
    onReload: () => undefined,
    onLoadMore: () => undefined,
    onPromote: async () => undefined,
    onDismiss: async () => undefined,
    onDefer: async () => undefined,
    onRetryAnalysis: async () => undefined,
    onOpenTask: async () => undefined,
  };
}

function renderDecisionInbox(
  overrides: Partial<ComponentProps<typeof DecisionInboxWorkspace>> = {},
): string {
  const props: ComponentProps<typeof DecisionInboxWorkspace> = {
    recommendations: [],
    inflowItems: [],
    itsmCandidates: [],
    loading: false,
    error: undefined,
    inflowSaving: false,
    gmailReview: emptyGmailReview(),
    onOpenConversation: () => undefined,
    onOpenTask: async () => undefined,
    onPromoteInflow: async () => undefined,
    onDismissInflow: async () => undefined,
    onRetryInflowAnalysis: async () => undefined,
    onRetryInflowCompletion: async () => undefined,
    onConfirmItsm: async () => undefined,
    onDecide: async () => true,
    onRetryAnalysis: async () => true,
    ...overrides,
  };
  return renderToStaticMarkup(createElement(DecisionInboxWorkspace, props));
}
