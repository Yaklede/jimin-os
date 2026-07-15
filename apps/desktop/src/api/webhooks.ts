import { PlanningRequestError } from "./planning";

export type ProjectWebhookEvent =
  | "project.updated"
  | "project.deleted"
  | "task.created"
  | "task.updated"
  | "task.completed"
  | "task.restored"
  | "task.deleted";

export interface ProjectWebhook {
  id: string;
  projectId: string;
  url: string;
  events: ProjectWebhookEvent[];
  hasAuthentication: boolean;
  enabled: boolean;
  version: number;
}

export interface WebhookDelivery {
  id: string;
  webhookId: string;
  eventType: string;
  status: "queued" | "sending" | "retry_wait" | "delivered" | "failed";
  attemptCount: number;
  responseCode: number | null;
  errorCode: string | null;
  createdAt: string;
  deliveredAt: string | null;
}

export type WebhookAuthorizationMode = "keep" | "replace" | "remove";

type ListResponse<T> = { items: T[]; nextCursor: string | null };

export async function fetchProjectWebhooks(
  baseUrl: string,
  access: string,
  projectId: string,
): Promise<ProjectWebhook[]> {
  return requestList<ProjectWebhook>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(projectId)}/webhooks`,
  );
}

export async function fetchWebhookDeliveries(
  baseUrl: string,
  access: string,
  projectId: string,
): Promise<WebhookDelivery[]> {
  return requestList<WebhookDelivery>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(projectId)}/webhook-deliveries`,
  );
}

export async function createProjectWebhook(
  baseUrl: string,
  access: string,
  projectId: string,
  input: {
    url: string;
    events: ProjectWebhookEvent[];
    authorization?: string;
  },
): Promise<ProjectWebhook> {
  return requestJson<ProjectWebhook>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(projectId)}/webhooks`,
    "POST",
    {
      url: input.url,
      events: input.events,
      authorization: input.authorization || null,
    },
  );
}

export async function updateProjectWebhook(
  baseUrl: string,
  access: string,
  webhook: ProjectWebhook,
  input: {
    url: string;
    events: ProjectWebhookEvent[];
    enabled: boolean;
    authorizationMode: WebhookAuthorizationMode;
    authorization?: string;
  },
): Promise<ProjectWebhook> {
  return requestJson<ProjectWebhook>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(webhook.projectId)}/webhooks/${encodeURIComponent(webhook.id)}`,
    "PUT",
    {
      url: input.url,
      events: input.events,
      enabled: input.enabled,
      authorizationMode: input.authorizationMode,
      authorization:
        input.authorizationMode === "replace"
          ? input.authorization || null
          : null,
      expectedVersion: webhook.version,
    },
  );
}

export async function deleteProjectWebhook(
  baseUrl: string,
  access: string,
  webhook: ProjectWebhook,
): Promise<void> {
  await requestEmpty(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(webhook.projectId)}/webhooks/${encodeURIComponent(webhook.id)}`,
    "DELETE",
    { expectedVersion: webhook.version },
  );
}

export async function testProjectWebhook(
  baseUrl: string,
  access: string,
  webhook: ProjectWebhook,
): Promise<void> {
  await requestEmpty(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(webhook.projectId)}/webhooks/${encodeURIComponent(webhook.id)}/test`,
    "POST",
  );
}

export async function retryWebhookDelivery(
  baseUrl: string,
  access: string,
  projectId: string,
  deliveryId: string,
): Promise<void> {
  await requestEmpty(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(projectId)}/webhook-deliveries/${encodeURIComponent(deliveryId)}/retry`,
    "POST",
  );
}

async function requestList<T>(
  baseUrl: string,
  access: string,
  path: string,
): Promise<T[]> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    headers: headers(access),
  });
  const payload = await readJson(response);
  if (!response.ok || !isListResponse<T>(payload)) {
    throw errorFromStatus(response.status);
  }
  return payload.items;
}

async function requestJson<T>(
  baseUrl: string,
  access: string,
  path: string,
  method: "POST" | "PUT",
  body?: unknown,
): Promise<T> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    method,
    headers: headers(access, true),
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const payload = await readJson(response);
  if (!response.ok || !isRecord(payload)) {
    throw errorFromStatus(response.status);
  }
  return payload as T;
}

async function requestEmpty(
  baseUrl: string,
  access: string,
  path: string,
  method: "POST" | "DELETE",
  body?: unknown,
): Promise<void> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    method,
    headers: headers(access, body !== undefined),
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw errorFromStatus(response.status);
}

function headers(access: string, json = false): Record<string, string> {
  return {
    Accept: "application/json",
    Authorization: `Bearer ${access}`,
    ...(json ? { "Content-Type": "application/json" } : {}),
  };
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
