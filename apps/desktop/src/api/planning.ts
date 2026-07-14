import { isUuidV7 } from "../uuid";

export type SessionTokens = Record<"accessToken" | "refreshToken", string>;

type ClientPlatform = "macos" | "ios" | "android";

export interface ScheduleEntry {
  id: string;
  title: string;
  notes: string | null;
  startsAt: string;
  endsAt: string;
  timeZone: string;
  status: "confirmed" | "cancelled";
  source: "manual" | "google_calendar";
  editable: boolean;
  version: number;
}

export interface Task {
  id: string;
  projectId: string | null;
  title: string;
  notes: string | null;
  status: "open" | "completed" | "cancelled";
  priority: number;
  dueAt: string | null;
  completedAt: string | null;
  version: number;
}

export interface PlanningSnapshot {
  schedule: ScheduleEntry[];
  tasks: Task[];
  completedTasks: Task[];
}

interface DeviceSessionResponse extends SessionTokens {
  user: unknown;
  device: unknown;
  syncCursor: string;
}

interface ListResponse<T> {
  items: T[];
  nextCursor: string | null;
}

export class PlanningRequestError extends Error {
  readonly code: "unauthorized" | "invalid" | "conflict" | "unavailable";

  constructor(code: PlanningRequestError["code"]) {
    super(code);
    this.name = "PlanningRequestError";
    this.code = code;
  }
}

export async function bootstrapTrustedNetworkSession(
  baseUrl: string,
  deviceName: string,
  installationId: string,
): Promise<SessionTokens> {
  if (!isUuidV7(installationId) || !deviceName.trim()) {
    throw new PlanningRequestError("invalid");
  }
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/access/session`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify({
        installationId,
        platform: clientPlatformForUserAgent(navigator.userAgent),
        name: deviceName,
        appVersion: "0.1.0-dev",
        osVersion: navigator.platform,
      }),
    },
  );
  const body = await readJson(response);
  if (!response.ok) {
    throw errorFromStatus(response.status);
  }
  if (!isDeviceSessionResponse(body)) {
    throw new PlanningRequestError("unavailable");
  }
  return {
    ["accessToken"]: body.accessToken,
    ["refreshToken"]: body.refreshToken,
  };
}

export function clientPlatformForUserAgent(userAgent: string): ClientPlatform {
  const normalized = userAgent.toLowerCase();
  if (normalized.includes("android")) return "android";
  if (/iphone|ipad|ipod/.test(normalized)) return "ios";
  return "macos";
}

export async function refreshDeviceSession(
  baseUrl: string,
  refresh: string,
): Promise<SessionTokens> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}/v1/auth/refresh`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify({ ["refreshToken"]: refresh }),
  });
  const body = await readJson(response);
  if (!response.ok) {
    throw errorFromStatus(response.status);
  }
  if (!isDeviceSessionResponse(body)) {
    throw new PlanningRequestError("unavailable");
  }
  return {
    ["accessToken"]: body.accessToken,
    ["refreshToken"]: body.refreshToken,
  };
}

export async function fetchPlanning(
  baseUrl: string,
  access: string,
  from: Date,
  to: Date,
): Promise<PlanningSnapshot> {
  const headers = {
    Accept: "application/json",
    Authorization: `Bearer ${access}`,
  };
  const base = normalizeBaseUrl(baseUrl);
  const scheduleUrl = new URL(
    `${base}/v1/schedule-entries`,
    window.location.origin,
  );
  scheduleUrl.searchParams.set("from", from.toISOString());
  scheduleUrl.searchParams.set("to", to.toISOString());
  const completedTaskUrl = new URL(`${base}/v1/tasks`, window.location.origin);
  completedTaskUrl.searchParams.set("status", "completed");
  const [scheduleResponse, taskResponse, completedTaskResponse] =
    await Promise.all([
      fetch(scheduleUrl.toString(), { headers }),
      fetch(`${base}/v1/tasks`, { headers }),
      fetch(completedTaskUrl.toString(), { headers }),
    ]);
  const [scheduleBody, taskBody, completedTaskBody] = await Promise.all([
    readJson(scheduleResponse),
    readJson(taskResponse),
    readJson(completedTaskResponse),
  ]);
  if (!scheduleResponse.ok) throw errorFromStatus(scheduleResponse.status);
  if (!taskResponse.ok) throw errorFromStatus(taskResponse.status);
  if (!completedTaskResponse.ok) {
    throw errorFromStatus(completedTaskResponse.status);
  }
  if (
    !isListResponse<ScheduleEntry>(scheduleBody) ||
    !isListResponse<Task>(taskBody) ||
    !isListResponse<Task>(completedTaskBody)
  ) {
    throw new PlanningRequestError("unavailable");
  }
  return {
    schedule: scheduleBody.items,
    tasks: taskBody.items,
    completedTasks: completedTaskBody.items,
  };
}

export async function createTask(
  baseUrl: string,
  access: string,
  input: {
    title: string;
    notes?: string;
    priority: number;
    dueAt?: string;
    projectId?: string;
  },
): Promise<Task> {
  return request<Task>(baseUrl, access, "/v1/tasks", "POST", {
    projectId: input.projectId || null,
    title: input.title,
    notes: input.notes || null,
    priority: input.priority,
    dueAt: input.dueAt || null,
  });
}

export async function completeTask(
  baseUrl: string,
  access: string,
  task: Task,
): Promise<Task> {
  return request<Task>(
    baseUrl,
    access,
    `/v1/tasks/${task.id}/complete`,
    "POST",
    {
      expectedVersion: task.version,
    },
  );
}

export async function updateTask(
  baseUrl: string,
  access: string,
  task: Task,
  input: {
    title: string;
    notes?: string;
    status: Task["status"];
    priority: number;
    dueAt?: string;
  },
): Promise<Task> {
  return request<Task>(baseUrl, access, `/v1/tasks/${task.id}`, "PUT", {
    projectId: task.projectId,
    title: input.title,
    notes: input.notes || null,
    status: input.status,
    priority: input.priority,
    dueAt: input.dueAt || null,
    expectedVersion: task.version,
  });
}

export async function createScheduleEntry(
  baseUrl: string,
  access: string,
  input: { title: string; startsAt: string; endsAt: string; notes?: string },
): Promise<ScheduleEntry> {
  return request<ScheduleEntry>(
    baseUrl,
    access,
    "/v1/schedule-entries",
    "POST",
    {
      title: input.title,
      notes: input.notes || null,
      startsAt: new Date(input.startsAt).toISOString(),
      endsAt: new Date(input.endsAt).toISOString(),
      timeZone:
        Intl.DateTimeFormat().resolvedOptions().timeZone || "Asia/Seoul",
    },
  );
}

export async function updateScheduleEntry(
  baseUrl: string,
  access: string,
  entry: ScheduleEntry,
  input: { title: string; startsAt: string; endsAt: string; notes?: string },
): Promise<ScheduleEntry> {
  return request<ScheduleEntry>(
    baseUrl,
    access,
    `/v1/schedule-entries/${entry.id}`,
    "PUT",
    {
      title: input.title,
      notes: input.notes || null,
      startsAt: new Date(input.startsAt).toISOString(),
      endsAt: new Date(input.endsAt).toISOString(),
      timeZone:
        Intl.DateTimeFormat().resolvedOptions().timeZone || "Asia/Seoul",
      expectedVersion: entry.version,
    },
  );
}

export async function deleteScheduleEntry(
  baseUrl: string,
  access: string,
  entry: ScheduleEntry,
): Promise<void> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/schedule-entries/${entry.id}`,
    {
      method: "DELETE",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ expectedVersion: entry.version }),
    },
  );
  if (!response.ok) throw errorFromStatus(response.status);
}

async function request<T>(
  baseUrl: string,
  access: string,
  path: string,
  method: "POST" | "PUT",
  body: unknown,
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
  if (!response.ok || !isRecord(payload))
    throw errorFromStatus(response.status);
  return payload as T;
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

function isDeviceSessionResponse(
  value: unknown,
): value is DeviceSessionResponse {
  return (
    isRecord(value) &&
    typeof value.accessToken === "string" &&
    typeof value.refreshToken === "string"
  );
}

function isListResponse<T>(value: unknown): value is ListResponse<T> {
  return isRecord(value) && Array.isArray(value.items);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
