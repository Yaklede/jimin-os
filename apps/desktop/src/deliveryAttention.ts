import type { HomeSnapshot } from "./api/home";
import type { ProjectInflowItem } from "./api/googleChat";
import {
  inflowDeliverySteps,
  inflowRecoveryItems,
} from "./inflowDeliveryState";

export function inflowFailureKey(item: ProjectInflowItem): string | undefined {
  if (!inflowDeliverySteps(item).some((step) => step.state === "failed"))
    return undefined;
  return `chat:${item.projectId}:${item.conversationId ?? item.id}:${item.status}`;
}

export function manualDeliveryFailures(
  snapshot: Pick<HomeSnapshot, "deliveryRecovery" | "recentInflow"> | undefined,
): ReadonlySet<string> {
  const failed = new Set<string>();
  for (const item of snapshot?.deliveryRecovery ?? []) {
    if (item.status === "failed" && item.canRetry)
      failed.add(`delivery:${item.kind}:${item.id}`);
  }
  for (const item of inflowRecoveryItems(snapshot?.recentInflow ?? [])) {
    const key = inflowFailureKey(item);
    if (key) failed.add(key);
  }
  return failed;
}
