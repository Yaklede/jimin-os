import {
  PlanningRequestError,
  type ScheduleEntry,
  type Task,
} from "./planning";

export interface Recommendation {
  id: string;
  workspaceId: string | null;
  projectId: string | null;
  goalId: string | null;
  signalId: string | null;
  title: string;
  rationale: string;
  expectedEffect: string;
  riskSummary: string | null;
  confidence: number;
  urgency: number;
  impact: number;
  riskLevel: number;
  effortMinutes: number | null;
  suggestedActionKind:
    | "review"
    | "create_task"
    | "update_task"
    | "create_schedule"
    | "update_project"
    | "run_webhook"
    | "request_analysis"
    | null;
  suggestedEntityId: string | null;
  status:
    | "pending"
    | "approved"
    | "rejected"
    | "deferred"
    | "analysis_requested"
    | "executing"
    | "executed"
    | "failed"
    | "expired";
  validUntil: string | null;
  revisitAt: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
}

export interface HomeSnapshot {
  schedule: ScheduleEntry[];
  tasks: Task[];
  dueTasks: Task[];
  recommendations: Recommendation[];
}

export async function fetchHomeSnapshot(
  baseUrl: string,
  access: string,
  from: Date,
  to: Date,
): Promise<HomeSnapshot> {
  const url = new URL(`${normalizeBaseUrl(baseUrl)}/v1/home`, browserOrigin());
  url.searchParams.set("from", from.toISOString());
  url.searchParams.set("to", to.toISOString());

  const response = await fetch(url.toString(), {
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${access}`,
    },
  });
  const body = await readJson(response);
  if (!response.ok || !isHomeSnapshot(body)) {
    throw errorFromStatus(response.status);
  }
  return {
    schedule: body.schedule,
    tasks: body.tasks,
    dueTasks: Array.isArray(body.dueTasks) ? body.dueTasks : [],
    recommendations: Array.isArray(body.recommendations)
      ? body.recommendations
      : [],
  };
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/$/, "");
}

function browserOrigin(): string {
  return typeof window === "undefined"
    ? "https://jimin-os.local"
    : window.location.origin;
}

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json();
  } catch {
    return null;
  }
}

function errorFromStatus(status: number): PlanningRequestError {
  if (status === 401) return new PlanningRequestError("unauthorized");
  if (status === 409) return new PlanningRequestError("conflict");
  if (status >= 400 && status < 500) return new PlanningRequestError("invalid");
  return new PlanningRequestError("unavailable");
}

function isHomeSnapshot(value: unknown): value is HomeSnapshot {
  return (
    isRecord(value) &&
    Array.isArray(value.schedule) &&
    Array.isArray(value.tasks)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
