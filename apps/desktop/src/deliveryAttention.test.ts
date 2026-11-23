import { describe, expect, it } from "vitest";
import type { DeliveryRecovery } from "./api/deliveryRecovery";
import { designPreviewInflow } from "./design-preview-inflow";
import { inflowFailureKey, manualDeliveryFailures } from "./deliveryAttention";

const unresolvedDelivery: DeliveryRecovery = {
  id: "delivery",
  kind: "itsm",
  projectId: "project",
  title: "예시 ITSM 반영",
  status: "failed",
  canRetry: true,
  attemptCount: 3,
  createdAt: "2026-11-23T00:00:00Z",
};

describe("delivery attention", () => {
  it("counts only actionable external failures and deduplicates Chat by conversation", () => {
    const failed = {
      ...designPreviewInflow()[0],
      status: "promoted" as const,
      completionStatus: "failed" as const,
      completionReactionCompleted: true,
      completionReplyCompleted: false,
    };
    const snapshot = {
      deliveryRecovery: [
        unresolvedDelivery,
        {
          ...unresolvedDelivery,
          id: "retrying",
          status: "retry_wait" as const,
        },
        {
          ...unresolvedDelivery,
          id: "expired",
          kind: "push" as const,
          canRetry: false,
        },
      ],
      recentInflow: [
        failed,
        { ...failed, id: "same-source", version: failed.version + 1 },
        {
          ...failed,
          id: "not-sent",
          conversationId: "not-sent",
          completionStatus: "not_requested" as const,
        },
        {
          ...failed,
          id: "queued",
          conversationId: "queued",
          completionStatus: "pending" as const,
        },
      ],
    };
    expect([...manualDeliveryFailures(snapshot)]).toEqual([
      "delivery:itsm:delivery",
      inflowFailureKey(failed),
    ]);
  });

  it("ignores a stale failure superseded by a success but counts a failed dismissal reply", () => {
    const failed = {
      ...designPreviewInflow()[0],
      status: "promoted" as const,
      completionStatus: "failed" as const,
      completionReactionCompleted: false,
      completionReplyCompleted: false,
    };
    const dismissal = {
      ...failed,
      status: "dismissed" as const,
      dismissalReplyStatus: "failed" as const,
    };
    expect([
      ...manualDeliveryFailures({
        recentInflow: [
          failed,
          {
            ...failed,
            version: failed.version + 1,
            completionStatus: "sent",
            completionReactionCompleted: true,
            completionReplyCompleted: true,
          },
          dismissal,
        ],
      }),
    ]).toEqual([inflowFailureKey(dismissal)]);
    expect(manualDeliveryFailures(undefined).size).toBe(0);
  });
});
