export interface ProjectItsmConnection {
  id: string;
  projectId: string;
  enabled: boolean;
  confirmationStatus:
    "discovering" | "confirmation_required" | "confirmed" | "disabled";
  candidateProjectName: string | null;
  version: number;
}

export interface ProjectItsmConnectionSnapshot {
  available: boolean;
  item: ProjectItsmConnection | null;
}

export interface ProjectItsmDecisionCandidate {
  projectName: string;
  connection: ProjectItsmConnection;
}

export class ItsmRequestError extends Error {
  readonly code:
    "unauthorized" | "invalid" | "conflict" | "forbidden" | "unavailable";

  constructor(code: ItsmRequestError["code"]) {
    super(code);
    this.name = "ItsmRequestError";
    this.code = code;
  }
}

export async function fetchProjectItsmConnection(
  baseUrl: string,
  access: string,
  projectId: string,
): Promise<ProjectItsmConnectionSnapshot> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/projects/${encodeURIComponent(projectId)}/itsm-connection`,
    { headers: headers(access) },
  );
  const payload = await readJson(response);
  if (!response.ok || !isSnapshot(payload)) {
    throw errorFromStatus(response.status);
  }
  return {
    available: payload.available,
    item: payload.item ? safeConnection(payload.item) : null,
  };
}

export async function connectProjectItsm(
  baseUrl: string,
  access: string,
  projectId: string,
): Promise<ProjectItsmConnection> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/projects/${encodeURIComponent(projectId)}/itsm-connection`,
    {
      method: "POST",
      headers: headers(access, true),
      body: JSON.stringify({ enabled: true }),
    },
  );
  const payload = await readJson(response);
  if (!response.ok || !isConnection(payload)) {
    throw errorFromStatus(response.status);
  }
  return safeConnection(payload);
}

export async function disconnectProjectItsm(
  baseUrl: string,
  access: string,
  connection: ProjectItsmConnection,
): Promise<void> {
  const url = new URL(
    `${normalizeBaseUrl(baseUrl)}/v1/projects/${encodeURIComponent(connection.projectId)}/itsm-connection`,
    browserOrigin(),
  );
  url.searchParams.set("expectedConnectionId", connection.id);
  url.searchParams.set("expectedVersion", String(connection.version));
  const response = await fetch(url, {
    method: "DELETE",
    headers: headers(access),
  });
  if (!response.ok) throw errorFromStatus(response.status);
}

export async function confirmProjectItsm(
  baseUrl: string,
  access: string,
  connection: ProjectItsmConnection,
): Promise<ProjectItsmConnection> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/projects/${encodeURIComponent(connection.projectId)}/itsm-connection/confirm`,
    {
      method: "POST",
      headers: headers(access, true),
      body: JSON.stringify({
        expectedConnectionId: connection.id,
        expectedVersion: connection.version,
      }),
    },
  );
  const payload = await readJson(response);
  if (!response.ok || !isConnection(payload)) {
    throw errorFromStatus(response.status);
  }
  return safeConnection(payload);
}

function safeConnection(value: ProjectItsmConnection): ProjectItsmConnection {
  return {
    id: value.id,
    projectId: value.projectId,
    enabled: value.enabled,
    confirmationStatus: value.confirmationStatus,
    candidateProjectName: value.candidateProjectName,
    version: value.version,
  };
}

function isSnapshot(value: unknown): value is ProjectItsmConnectionSnapshot {
  return (
    isRecord(value) &&
    typeof value.available === "boolean" &&
    (value.item === null || isConnection(value.item))
  );
}

function isConnection(value: unknown): value is ProjectItsmConnection {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    value.id.length > 0 &&
    typeof value.projectId === "string" &&
    value.projectId.length > 0 &&
    typeof value.enabled === "boolean" &&
    isConfirmationStatus(value.confirmationStatus) &&
    (value.candidateProjectName === null ||
      typeof value.candidateProjectName === "string") &&
    Number.isSafeInteger(value.version) &&
    Number(value.version) > 0
  );
}

function isConfirmationStatus(
  value: unknown,
): value is ProjectItsmConnection["confirmationStatus"] {
  return (
    value === "discovering" ||
    value === "confirmation_required" ||
    value === "confirmed" ||
    value === "disabled"
  );
}

function headers(access: string, json = false): HeadersInit {
  return {
    Authorization: `Bearer ${access}`,
    ...(json ? { "Content-Type": "application/json" } : {}),
  };
}

async function readJson(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

function errorFromStatus(status: number): ItsmRequestError {
  if (status === 401) return new ItsmRequestError("unauthorized");
  if (status === 403) return new ItsmRequestError("forbidden");
  if (status === 409) return new ItsmRequestError("conflict");
  if (status >= 500 || status === 0) return new ItsmRequestError("unavailable");
  return new ItsmRequestError("invalid");
}

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.replace(/\/+$/, "");
}

function browserOrigin(): string {
  return typeof window === "undefined"
    ? "http://localhost"
    : window.location.origin;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
