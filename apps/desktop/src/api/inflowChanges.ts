import type { ProjectInflowItem } from "./googleChat";
import { PlanningRequestError, type Task } from "./planning";
export async function approveInflowChange(
  base: string,
  token: string,
  item: ProjectInflowItem,
  task: Task,
  notes: string,
  dueAt: string | null,
): Promise<void> {
  const response = await fetch(
    `${base.replace(/\/$/, "")}/v1/inflow/${encodeURIComponent(item.id)}/change-approval`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        expectedItemVersion: item.version,
        taskId: task.id,
        expectedTaskVersion: task.version,
        sourceRevision: item.sourceRevision,
        notes,
        dueAt,
      }),
    },
  );
  if (!response.ok)
    throw new PlanningRequestError(
      response.status === 409
        ? "conflict"
        : response.status === 401
          ? "unauthorized"
          : "unavailable",
    );
}
