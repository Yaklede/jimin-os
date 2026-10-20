import { describe, expect, it } from "vitest";
import type { ProjectInflowItem } from "./api/googleChat";
import { designPreviewInflow } from "./design-preview-inflow";
import { inflowRecoveryCopy as text } from "./copy/inflowRecovery";
import {
  canRetryInflowDelivery,
  inflowDeliveryHelp,
  inflowDeliverySteps,
  inflowRecoveryItems,
} from "./inflowDeliveryState";

function item(overrides: Partial<ProjectInflowItem> = {}): ProjectInflowItem {
  return {
    ...designPreviewInflow()[0],
    status: "promoted",
    promotedTaskId: "task",
    completionStatus: "failed",
    ...overrides,
  };
}
describe("inflow delivery stages", () => {
  it("keeps task saving successful when the reply fails", () => {
    expect(
      inflowDeliverySteps(item({ completionReactionCompleted: true })).map(
        (step) => step.state,
      ),
    ).toEqual(["done", "done", "failed"]);
  });
  it("keeps the successful reply when the check fails", () => {
    expect(
      inflowDeliverySteps(item({ completionReplyCompleted: true })).map(
        (step) => step.state,
      ),
    ).toEqual(["done", "failed", "done"]);
  });
  it("does not offer a new retry while a delivery is pending or done", () => {
    expect(canRetryInflowDelivery(item({ completionStatus: "pending" }))).toBe(
      false,
    );
    expect(
      canRetryInflowDelivery(
        item({
          completionReactionCompleted: true,
          completionReplyCompleted: true,
        }),
      ),
    ).toBe(false);
    expect(
      canRetryInflowDelivery(item({ completionStatus: "not_requested" })),
    ).toBe(true);
  });
  it("does not infer successful stages from an inconsistent aggregate status", () => {
    expect(
      inflowDeliverySteps(item({ completionStatus: "sent" })).map(
        (step) => step.state,
      ),
    ).toEqual(["done", "pending", "pending"]);
  });
  it("excludes processed history, deduplicates conversations and prioritizes failures", () => {
    const failed = item();
    const newer = { ...failed, version: 2 };
    const pending = item({
      id: "pending",
      conversationId: "pending",
      completionStatus: "pending",
    });
    expect(
      inflowRecoveryItems([
        pending,
        failed,
        newer,
        item({
          id: "done",
          conversationId: "done",
          completionReactionCompleted: true,
          completionReplyCompleted: true,
        }),
        item({ status: "pending" }),
        item({ status: "dismissed", dismissalReplyStatus: null }),
      ]),
    ).toEqual([newer, pending]);
  });
  it("separates dismissal saving and optional reason reply", () => {
    expect(
      inflowDeliverySteps(
        item({ status: "dismissed", dismissalReplyStatus: "failed" }),
      ).map((step) => step.state),
    ).toEqual(["done", "failed"]);
    expect(
      inflowDeliverySteps(
        item({ status: "dismissed", dismissalReplyStatus: null }),
      ),
    ).toHaveLength(1);
  });
  it("does not resurrect an older failure after the latest version succeeds", () => {
    const failed = item({ version: 1 });
    const done = item({
      version: 2,
      completionStatus: "sent",
      completionReactionCompleted: true,
      completionReplyCompleted: true,
    });
    expect(inflowRecoveryItems([failed, done])).toEqual([]);
    expect(inflowRecoveryItems([done, failed])).toEqual([]);
  });
  it("gives actionable guidance without exposing internal provider errors", () => {
    expect(
      inflowDeliveryHelp(
        item({
          completionErrorCode: "google_chat.completion_reply_unauthenticated",
        }),
      ),
    ).toBe(text.reconnectHelp);
    expect(
      inflowDeliveryHelp(
        item({
          completionErrorCode:
            "google_chat.completion_reaction_permission_denied",
        }),
      ),
    ).toBe(text.permissionHelp);
    expect(
      inflowDeliveryHelp(
        item({ completionErrorCode: "google_chat.completion_reply_not_found" }),
      ),
    ).toBe(text.missingHelp);
  });
});
