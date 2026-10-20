import { PlanningRequestError } from "./planning";
export interface DeliveryRecovery {
  id: string;
  kind: "webhook" | "push" | "itsm";
  projectId: string | null;
  title: string;
  status: "failed" | "retry_wait" | "queued" | "sending";
  canRetry: boolean;
  attemptCount: number;
  createdAt: string;
  errorCode?: string | null;
  referenceUrl?: string | null;
}
export async function retryDeliveryRecovery(
  base: string,
  token: string,
  item: DeliveryRecovery,
): Promise<void> {
  const response = await fetch(
    `${base.replace(/\/$/, "")}/v1/delivery-recovery/${item.kind}/${encodeURIComponent(item.id)}/retry`,
    { method: "POST", headers: { Authorization: `Bearer ${token}` } },
  );
  if (!response.ok)
    throw new PlanningRequestError(
      response.status === 401
        ? "unauthorized"
        : response.status === 409
          ? "conflict"
          : "unavailable",
    );
}
