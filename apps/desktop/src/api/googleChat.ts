export type GoogleChatAccountStatus =
  | "connecting"
  | "active"
  | "reauth_required"
  | "revoking"
  | "revoked"
  | "error";

export interface GoogleChatAccount {
  id: string;
  email: string;
  status: GoogleChatAccountStatus;
  lastSuccessfulSyncAt: string | null;
  lastErrorCode: string | null;
  reauthRequired: boolean;
  version: number;
}

export interface GoogleChatConnectionList {
  available: boolean;
  items: GoogleChatAccount[];
}

export interface GoogleChatSpace {
  name: string;
  displayName: string;
}

export interface ProjectGoogleChatSource {
  id: string;
  projectId: string;
  accountId: string;
  accountEmail: string;
  spaceName: string;
  displayName: string;
  enabled: boolean;
  acknowledgeWithReaction: boolean;
  lastSuccessfulSyncAt: string | null;
  lastErrorCode: string | null;
  version: number;
}

export type ProjectInflowStatus = "pending" | "promoted" | "dismissed";

export interface ProjectInflowItem {
  id: string;
  projectId: string;
  sourceId: string;
  sourceName: string;
  senderName: string | null;
  contentText: string;
  receivedAt: string;
  status: ProjectInflowStatus;
  promotedTaskId: string | null;
  acknowledged: boolean;
  version: number;
}

export interface GoogleChatAuthorization {
  authorizationId: string;
  authorizationUrl: string;
  expiresAt: string;
}

export async function fetchGoogleChatConnections(
  baseUrl: string,
  access: string,
): Promise<GoogleChatConnectionList> {
  return request<GoogleChatConnectionList>(
    baseUrl,
    access,
    "/v1/google-chat/connections",
  );
}

export async function startGoogleChatAuthorization(
  baseUrl: string,
  access: string,
): Promise<GoogleChatAuthorization> {
  return request<GoogleChatAuthorization>(
    baseUrl,
    access,
    "/v1/google-chat/connections/authorizations",
    { method: "POST", body: JSON.stringify({ clientKind: clientKind() }) },
  );
}

export async function deleteGoogleChatConnection(
  baseUrl: string,
  access: string,
  account: GoogleChatAccount,
): Promise<void> {
  const path = `/v1/google-chat/connections/${encodeURIComponent(account.id)}?expectedVersion=${account.version}`;
  await request<void>(baseUrl, access, path, { method: "DELETE" }, true);
}

export async function fetchGoogleChatSpaces(
  baseUrl: string,
  access: string,
  accountId: string,
): Promise<GoogleChatSpace[]> {
  const response = await request<{ items: GoogleChatSpace[] }>(
    baseUrl,
    access,
    `/v1/google-chat/connections/${encodeURIComponent(accountId)}/spaces`,
  );
  return response.items;
}

export async function fetchProjectGoogleChatSources(
  baseUrl: string,
  access: string,
  projectId: string,
): Promise<ProjectGoogleChatSource[]> {
  const response = await request<{ items: ProjectGoogleChatSource[] }>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(projectId)}/google-chat-sources`,
  );
  return response.items;
}

export async function createProjectGoogleChatSource(
  baseUrl: string,
  access: string,
  projectId: string,
  input: {
    accountId: string;
    spaceName: string;
    displayName: string;
    acknowledgeWithReaction: boolean;
  },
): Promise<ProjectGoogleChatSource> {
  return request<ProjectGoogleChatSource>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(projectId)}/google-chat-sources`,
    { method: "POST", body: JSON.stringify(input) },
  );
}

export async function deleteProjectGoogleChatSource(
  baseUrl: string,
  access: string,
  source: ProjectGoogleChatSource,
): Promise<void> {
  const path = `/v1/projects/${encodeURIComponent(source.projectId)}/google-chat-sources/${encodeURIComponent(source.id)}?expectedVersion=${source.version}`;
  await request<void>(baseUrl, access, path, { method: "DELETE" }, true);
}

export async function syncProjectGoogleChatSource(
  baseUrl: string,
  access: string,
  source: ProjectGoogleChatSource,
): Promise<ProjectGoogleChatSource[]> {
  const response = await request<{ items: ProjectGoogleChatSource[] }>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(source.projectId)}/google-chat-sources/${encodeURIComponent(source.id)}/sync`,
    { method: "POST" },
  );
  return response.items;
}

export async function fetchProjectInflow(
  baseUrl: string,
  access: string,
  projectId: string,
  status: ProjectInflowStatus | "all" = "all",
): Promise<ProjectInflowItem[]> {
  const path = `/v1/projects/${encodeURIComponent(projectId)}/inflow?status=${status}`;
  const response = await request<{ items: ProjectInflowItem[] }>(
    baseUrl,
    access,
    path,
  );
  return response.items;
}

export async function decideProjectInflow(
  baseUrl: string,
  access: string,
  item: ProjectInflowItem,
  input:
    | { decision: "dismiss" }
    | { decision: "promote"; title: string; priority: number; dueAt?: string },
): Promise<ProjectInflowItem> {
  return request<ProjectInflowItem>(
    baseUrl,
    access,
    `/v1/projects/${encodeURIComponent(item.projectId)}/inflow/${encodeURIComponent(item.id)}/decision`,
    {
      method: "POST",
      body: JSON.stringify({ ...input, expectedVersion: item.version }),
    },
  );
}

async function request<T>(
  baseUrl: string,
  access: string,
  path: string,
  init: RequestInit = {},
  emptyResponse = false,
): Promise<T> {
  const response = await fetch(`${baseUrl.replace(/\/$/, "")}${path}`, {
    ...init,
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${access}`,
      ...(init.body ? { "Content-Type": "application/json" } : {}),
      ...init.headers,
    },
  });
  if (!response.ok)
    throw new Error(`Google Chat request failed: ${response.status}`);
  if (emptyResponse || response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

function clientKind(): "macos" | "android" | "ios" {
  const platform = navigator.userAgent.toLowerCase();
  if (platform.includes("android")) return "android";
  if (platform.includes("iphone") || platform.includes("ipad")) return "ios";
  return "macos";
}
