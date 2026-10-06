import { PlanningRequestError } from "./planning";

export type ScheduledWorkScope =
  "today" | "tomorrow" | "today_tomorrow" | "overdue" | "all_open";
export type ScheduledWorkMessageDetail = "title_only" | "title_and_details";
export interface ScheduledWorkDefinition {
  title: string;
  workspaceId: string;
  projectId: string | null;
  webhookId: string | null;
  weekdays: number[];
  time: string;
  followUpTime: string | null;
  timeZone: "Asia/Seoul";
  taskScope: ScheduledWorkScope;
  includeOverdue: boolean;
  assigneeNames: string[];
  destination: "google_chat" | "in_app";
  mentionAssignees: boolean;
  mentionNames: string[];
  includeSchedules: boolean;
  /** Older stored rules omit this field and use title_only. */
  messageDetail?: ScheduledWorkMessageDetail;
}

export function normalizeScheduledWorkDefinition(
  definition: ScheduledWorkDefinition,
): ScheduledWorkDefinition {
  return {
    ...definition,
    messageDetail: definition.messageDetail ?? "title_only",
  };
}
export interface ScheduledWork {
  id: string;
  definition: ScheduledWorkDefinition;
  enabled: boolean;
  nextRunAt: string;
  version: number;
}
export interface ScheduledWorkRun {
  id: string;
  scheduledWorkId: string;
  scheduledFor: string;
  kind: "primary" | "followup" | "manual";
  status: "delivering" | "completed" | "skipped" | "failed";
  reason: string | null;
  taskIds: string[];
  messages: string[];
  startedAt: string;
}
export interface ScheduledWorkPreview {
  taskIds: string[];
  messages: string[];
  warnings: string[];
  scheduleCount: number;
  nextRunAt: string;
}
export interface ScheduledWorkRunPage {
  items: ScheduledWorkRun[];
  nextCursor: string | null;
}
export async function scheduledRequest<T>(
  base: string,
  token: string,
  path = "",
  body?: unknown,
  method = "GET",
): Promise<T> {
  const response = await fetch(
    `${base.replace(/\/$/, "")}/v1/scheduled-work${path}`,
    {
      method,
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
