import type { ProjectInflowItem } from "./api/googleChat";
import { inflowRecoveryCopy as text } from "./copy/inflowRecovery";

export type InflowDeliveryState =
  "done" | "pending" | "failed" | "not_requested";
export type InflowDeliveryStep = {
  key: "saved" | "reaction" | "reply";
  label: string;
  state: InflowDeliveryState;
};

export function inflowDeliverySteps(
  item: ProjectInflowItem,
): InflowDeliveryStep[] {
  if (item.status === "pending") return [];
  const steps: InflowDeliveryStep[] = [
    {
      key: "saved",
      label: item.status === "promoted" ? text.savedTask : text.savedDismissal,
      state: "done",
    },
  ];
  if (item.status === "dismissed") {
    if (item.dismissalReplyStatus)
      steps.push({
        key: "reply",
        label: text.dismissalReply,
        state:
          item.dismissalReplyStatus === "sent"
            ? "done"
            : item.dismissalReplyStatus,
      });
    return steps;
  }
  const incomplete: InflowDeliveryState =
    item.completionStatus === "not_requested"
      ? "not_requested"
      : item.completionStatus === "failed"
        ? "failed"
        : "pending";
  // Individual acknowledgements, not a combined status, prove each stage succeeded.
  steps.push(
    {
      key: "reaction",
      label: text.reaction,
      state: item.completionReactionCompleted ? "done" : incomplete,
    },
    {
      key: "reply",
      label: text.reply,
      state: item.completionReplyCompleted ? "done" : incomplete,
    },
  );
  return steps;
}

export function inflowNeedsDeliveryAttention(item: ProjectInflowItem): boolean {
  return inflowDeliverySteps(item).some((step) => step.state !== "done");
}

export function canRetryInflowDelivery(item: ProjectInflowItem): boolean {
  return inflowDeliverySteps(item).some(
    (step) => step.state === "failed" || step.state === "not_requested",
  );
}

export function inflowDeliveryHelp(item: ProjectInflowItem): string {
  if (item.status === "dismissed")
    return item.dismissalReplyStatus === "pending"
      ? text.pendingHelp
      : text.dismissalHelp;
  if (item.completionStatus === "not_requested") return text.noRequestHelp;
  if (!canRetryInflowDelivery(item)) return text.pendingHelp;
  const code = item.completionErrorCode ?? "";
  if (/unauthenticated|reauth|write_scope_missing/.test(code))
    return text.reconnectHelp;
  if (/permission_denied/.test(code)) return text.permissionHelp;
  if (/not_found/.test(code)) return text.missingHelp;
  return text.retryHelp;
}

export function inflowRecoveryItems(
  items: ProjectInflowItem[],
): ProjectInflowItem[] {
  const unique = new Map<string, ProjectInflowItem>();
  for (const item of items) {
    const key = `${item.projectId}:${item.conversationId ?? item.id}:${item.status}`;
    const previous = unique.get(key);
    if (!previous || item.version > previous.version) unique.set(key, item);
  }
  return [...unique.values()]
    .filter(inflowNeedsDeliveryAttention)
    .sort(
      (a, b) =>
        Number(canRetryInflowDelivery(b)) - Number(canRetryInflowDelivery(a)) ||
        b.receivedAt.localeCompare(a.receivedAt),
    );
}
