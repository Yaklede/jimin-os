import { PlanningRequestError } from "./planning";

export interface Goal {
  id: string;
  workspaceId: string | null;
  projectId: string | null;
  title: string;
  desiredOutcome: string;
  status: "active" | "paused" | "achieved" | "cancelled";
  targetAt: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
}

type GoalInput = {
  workspaceId?: string;
  projectId?: string;
  title: string;
  desiredOutcome: string;
  targetAt?: string;
};

type ListResponse<T> = { items: T[]; nextCursor: string | null };

export async function fetchGoals(
  baseUrl: string,
  access: string,
): Promise<Goal[]> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}/v1/goals`, {
    headers: { Accept: "application/json", Authorization: `Bearer ${access}` },
  });
  const body = await readJson(response);
  if (!response.ok || !isListResponse<Goal>(body)) {
    throw errorFromStatus(response.status);
  }
  return body.items;
}

export async function createGoal(
  baseUrl: string,
  access: string,
  input: GoalInput,
): Promise<Goal> {
  return requestGoal(baseUrl, access, "/v1/goals", "POST", {
    workspaceId: input.workspaceId ?? null,
    projectId: input.projectId ?? null,
    title: input.title,
    desiredOutcome: input.desiredOutcome,
    targetAt: input.targetAt ?? null,
  });
}

export async function updateGoal(
  baseUrl: string,
  access: string,
  goal: Goal,
  input: GoalInput & { status: Goal["status"] },
): Promise<Goal> {
  return requestGoal(
    baseUrl,
    access,
    `/v1/goals/${encodeURIComponent(goal.id)}`,
    "PUT",
    {
      workspaceId: input.workspaceId ?? null,
      projectId: input.projectId ?? null,
      title: input.title,
      desiredOutcome: input.desiredOutcome,
      status: input.status,
      targetAt: input.targetAt ?? null,
      expectedVersion: goal.version,
    },
  );
}

async function requestGoal(
  baseUrl: string,
  access: string,
  path: string,
  method: "POST" | "PUT",
  body: unknown,
): Promise<Goal> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    method,
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${access}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  const payload = await readJson(response);
  if (!response.ok || !isGoal(payload)) {
    throw errorFromStatus(response.status);
  }
  return payload;
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/$/, "");
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

function isListResponse<T>(value: unknown): value is ListResponse<T> {
  return isRecord(value) && Array.isArray(value.items);
}

function isGoal(value: unknown): value is Goal {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.title === "string" &&
    typeof value.desiredOutcome === "string" &&
    typeof value.status === "string" &&
    typeof value.version === "number"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
