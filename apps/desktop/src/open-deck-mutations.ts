import { PlanningRequestError } from "./api/planning";
export interface DeckMutation {
  mutationId: string;
  operation: "create-schedule" | "complete-task";
  itemId?: string;
  expectedVersion?: number;
  title?: string;
  notes?: string;
  startsAt?: string;
  endsAt?: string;
  completionNote?: string;
  retry?: boolean;
}
export interface DeckMutationOutcome {
  entityId?: string;
  error?: string;
  alreadyCompleted?: boolean;
}
class MutationFailure extends Error {}
export async function executeDeckMutation(
  base: string,
  access: string,
  m: DeckMutation,
  fetcher = fetch,
): Promise<DeckMutationOutcome> {
  let attempted = false;
  const request = async (
    path: string,
    body?: unknown,
  ): Promise<Record<string, unknown>> => {
    if (body) attempted = true;
    const response = await fetcher(base.replace(/\/$/, "") + path, {
      method: body ? "POST" : "GET",
      headers: {
        Authorization: "Bearer " + access,
        "Content-Type": "application/json",
      },
      body: body ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(10000),
    });
    if (response.status === 401) throw new PlanningRequestError("unauthorized");
    if (!response.ok)
      throw new MutationFailure(
        response.status === 409
          ? "CONFLICT"
          : response.status === 404
            ? "NOT_FOUND"
            : response.status === 400 || response.status === 422
              ? "INVALID"
              : body
                ? "UNKNOWN_RESULT"
                : "UNAVAILABLE",
      );
    return (await response.json()) as Record<string, unknown>;
  };
  try {
    if (m.operation === "create-schedule") {
      const start = Date.parse(m.startsAt || ""),
        end = Date.parse(m.endsAt || "");
      if (
        !m.title?.trim() ||
        m.title.length > 160 ||
        !Number.isFinite(start) ||
        !Number.isFinite(end) ||
        end <= start
      )
        throw new MutationFailure("INVALID");
      const query = new URLSearchParams({
        from: new Date(start - 86400000).toISOString(),
        to: new Date(end + 86400000).toISOString(),
      });
      const rows = await request("/v1/schedule-entries?" + query);
      const existing = Array.isArray(rows.items)
        ? rows.items.find((v: Record<string, unknown>) => v.id === m.mutationId)
        : undefined;
      if (existing) {
        if (
          existing.title !== m.title.trim() ||
          Date.parse(existing.startsAt) !== start ||
          Date.parse(existing.endsAt) !== end
        )
          throw new MutationFailure("CONFLICT");
        return { entityId: m.mutationId };
      }
      const created = await request("/v1/schedule-entries", {
        clientMutationId: m.mutationId,
        title: m.title.trim(),
        notes: m.notes?.trim() || null,
        startsAt: new Date(start).toISOString(),
        endsAt: new Date(end).toISOString(),
        timeZone: "Asia/Seoul",
        projectId: null,
        taskId: null,
      });
      if (created.id !== m.mutationId)
        throw new MutationFailure("UNKNOWN_RESULT");
      return { entityId: m.mutationId };
    }
    if (
      m.operation !== "complete-task" ||
      !m.itemId ||
      !Number.isInteger(m.expectedVersion)
    )
      throw new MutationFailure("INVALID");
    const task = await request("/v1/tasks/" + encodeURIComponent(m.itemId));
    if (task.id !== m.itemId) throw new MutationFailure("NOT_FOUND");
    if (task.status === "completed")
      return { entityId: m.itemId, alreadyCompleted: true };
    if (task.status !== "open" || task.version !== m.expectedVersion)
      throw new MutationFailure("CONFLICT");
    const completed = await request(
      "/v1/tasks/" + encodeURIComponent(m.itemId) + "/complete",
      {
        expectedVersion: m.expectedVersion,
        ...(m.completionNote?.trim()
          ? { completionNote: m.completionNote.trim() }
          : {}),
      },
    );
    if (completed.id !== m.itemId || completed.status !== "completed")
      throw new MutationFailure("UNKNOWN_RESULT");
    return { entityId: m.itemId };
  } catch (cause) {
    if (cause instanceof PlanningRequestError) throw cause;
    return {
      error:
        cause instanceof MutationFailure
          ? cause.message
          : attempted
            ? "UNKNOWN_RESULT"
            : "UNAVAILABLE",
    };
  }
}
