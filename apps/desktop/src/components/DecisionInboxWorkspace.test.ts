import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { type ProjectInflowItem } from "../api/googleChat";
import { type Recommendation } from "../api/home";
import { copy } from "../copy";
import {
  DecisionInboxWorkspace,
  inflowDecisionSummary,
  isConversationDecision,
  isDecisionActionableNow,
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
        onOpenConversation: () => undefined,
        onOpenProjectInflow: async () => undefined,
        onConfirmItsm: async () => undefined,
        onDecide: async () => true,
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

  it("only treats ready new tasks as inflow decisions", () => {
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
        promotedTaskId: "019f68cb-9400-7000-8000-000000000033",
      }),
    ).toBe(false);
  });
});
