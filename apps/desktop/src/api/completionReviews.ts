import { PlanningRequestError } from "./planning";

export interface CompletionReview {
  id: string;
  taskId: string;
  projectId: string | null;
  projectTitle: string | null;
  title: string;
  taskNotes: string | null;
  assigneeName: string | null;
  dueAt: string | null;
  reporterName: string;
  completionNote: string | null;
  reportedTaskVersion: number;
  taskVersion: number;
  version: number;
  createdAt: string;
}
export interface CompletionReviewPage {
  items: CompletionReview[];
  nextCursor: string | null;
}
export async function completionReviewRequest<T>(
  base: string,
  token: string,
  path = "",
  body?: unknown,
): Promise<T> {
  const response = await fetch(
    `${base.replace(/\/$/, "")}/v1/task-completion-reviews${path}`,
    {
      method: body === undefined ? "GET" : "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        ...(body === undefined ? {} : { "Content-Type": "application/json" }),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    },
  );
  if (!response.ok)
    throw new PlanningRequestError(
      response.status === 401
        ? "unauthorized"
        : response.status === 409
          ? "conflict"
          : response.status === 400
            ? "invalid"
            : "unavailable",
    );
  return response.json() as Promise<T>;
}
