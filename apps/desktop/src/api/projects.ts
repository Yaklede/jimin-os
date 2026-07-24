import { PlanningRequestError, type Task } from "./planning";

export interface Workspace {
  id: string;
  scope: "personal" | "company";
  name: string;
  version: number;
}

export interface Project {
  id: string;
  workspaceId: string;
  title: string;
  objective: string | null;
  status: "active" | "paused" | "completed";
  managementMode: "completion" | "operation";
  reportingEnabled: boolean;
  staleThresholdDays: number;
  riskLevel: number;
  nextAction: string | null;
  dueAt: string | null;
  openTaskCount: number;
  totalTaskCount: number;
  completedTaskCount: number;
  overdueTaskCount: number;
  unassignedTaskCount: number;
  progressPercent: number;
  weeklyCreatedTaskCount: number;
  weeklyCompletedTaskCount: number;
  backlogDelta: number;
  staleTaskCount: number;
  averageCycleTimeHours: number;
  onTimeCompletionPercent: number | null;
  health:
    | "on_track"
    | "at_risk"
    | "needs_attention"
    | "needs_plan"
    | "ready_to_complete"
    | "paused"
    | "completed";
  version: number;
}

type ListResponse<T> = { items: T[]; nextCursor: string | null };

export async function fetchWorkspaces(
  baseUrl: string,
  access: string,
): Promise<Workspace[]> {
  return requestList<Workspace>(baseUrl, access, "/v1/workspaces");
}

export async function fetchProjects(
  baseUrl: string,
  access: string,
  workspaceId: string,
): Promise<Project[]> {
  const url = new URL(
    `${normalizeBaseUrl(baseUrl)}/v1/projects`,
    browserOrigin(),
  );
  url.searchParams.set("workspaceId", workspaceId);
  return requestListFromUrl<Project>(url, access);
}

export async function fetchProjectTasks(
  baseUrl: string,
  access: string,
  projectId: string,
): Promise<Task[]> {
  const url = new URL(`${normalizeBaseUrl(baseUrl)}/v1/tasks`, browserOrigin());
  url.searchParams.set("projectId", projectId);
  url.searchParams.set("status", "all");
  return requestListFromUrl<Task>(url, access);
}

export async function createProject(
  baseUrl: string,
  access: string,
  input: {
    workspaceId: string;
    title: string;
    objective?: string;
    managementMode: Project["managementMode"];
    reportingEnabled: boolean;
    staleThresholdDays: number;
    riskLevel: number;
    nextAction?: string;
    dueAt?: string;
  },
): Promise<Project> {
  return request<Project>(
    baseUrl,
    access,
    "/v1/projects",
    {
      workspaceId: input.workspaceId,
      title: input.title,
      objective: input.objective || null,
      managementMode: input.managementMode,
      reportingEnabled: input.reportingEnabled,
      staleThresholdDays: input.staleThresholdDays,
      riskLevel: input.riskLevel,
      nextAction: input.nextAction || null,
      dueAt: input.dueAt || null,
    },
    "POST",
  );
}

export async function updateProject(
  baseUrl: string,
  access: string,
  project: Project,
  input: {
    title: string;
    objective?: string;
    status: Project["status"];
    managementMode: Project["managementMode"];
    reportingEnabled: boolean;
    staleThresholdDays: number;
    riskLevel: number;
    nextAction?: string;
    dueAt?: string;
  },
): Promise<Project> {
  return request<Project>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(project.id)}`,
    {
      title: input.title,
      objective: input.objective || null,
      status: input.status,
      managementMode: input.managementMode,
      reportingEnabled: input.reportingEnabled,
      staleThresholdDays: input.staleThresholdDays,
      riskLevel: input.riskLevel,
      nextAction: input.nextAction || null,
      dueAt: input.dueAt || null,
      expectedVersion: project.version,
    },
    "PUT",
  );
}

export async function deleteProject(
  baseUrl: string,
  access: string,
  project: Project,
): Promise<void> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/projects/${encodeURIComponent(project.id)}`,
    {
      method: "DELETE",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ expectedVersion: project.version }),
    },
  );
  if (!response.ok) throw errorFromStatus(response.status);
}

async function requestList<T>(
  baseUrl: string,
  access: string,
  path: string,
): Promise<T[]> {
  const url = new URL(`${normalizeBaseUrl(baseUrl)}${path}`, browserOrigin());
  return requestListFromUrl<T>(url, access);
}

async function requestListFromUrl<T>(url: URL, access: string): Promise<T[]> {
  const response = await fetch(url.toString(), {
    headers: { Accept: "application/json", Authorization: `Bearer ${access}` },
  });
  const body = await readJson(response);
  if (!response.ok || !isListResponse<T>(body)) {
    throw errorFromStatus(response.status);
  }
  return body.items;
}

async function request<T>(
  baseUrl: string,
  access: string,
  path: string,
  body: unknown,
  method: "POST" | "PUT",
): Promise<T> {
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
  if (!response.ok || !isRecord(payload)) {
    throw errorFromStatus(response.status);
  }
  return payload as T;
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

function isListResponse<T>(value: unknown): value is ListResponse<T> {
  return isRecord(value) && Array.isArray(value.items);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
