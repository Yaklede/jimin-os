import { PlanningRequestError, clientPlatformForUserAgent } from "./planning";

export type GoogleCalendarConnectionStatus =
  | "not_connected"
  | "connecting"
  | "active"
  | "reauth_required"
  | "revoking"
  | "revoked"
  | "error";

export interface GoogleCalendarConnection {
  available: boolean;
  status: GoogleCalendarConnectionStatus;
  email: string | null;
  grantedScopes: string[];
  lastSuccessfulSyncAt: string | null;
  lastErrorCode: string | null;
  reauthRequired: boolean;
  version: number | null;
}

export interface GoogleCalendarAuthorization {
  authorizationId: string;
  authorizationUrl: string;
  expiresAt: string;
}

export async function fetchGoogleCalendarConnection(
  baseUrl: string,
  access: string,
): Promise<GoogleCalendarConnection> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/calendar/connections/google`,
    { headers: requestHeaders(access) },
  );
  const body = await readJson(response);
  if (!response.ok || !isGoogleCalendarConnection(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function startGoogleCalendarAuthorization(
  baseUrl: string,
  access: string,
  userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent,
): Promise<GoogleCalendarAuthorization> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/calendar/connections/google/authorizations`,
    {
      method: "POST",
      headers: requestHeaders(access, true),
      body: JSON.stringify({
        clientKind: clientPlatformForUserAgent(userAgent),
      }),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isGoogleCalendarAuthorization(body)) {
    throw errorFromStatus(response.status);
  }
  if (!isTrustedGoogleAuthorizationUrl(body.authorizationUrl)) {
    throw new PlanningRequestError("unavailable");
  }
  return body;
}

export async function synchronizeGoogleCalendar(
  baseUrl: string,
  access: string,
): Promise<GoogleCalendarConnection> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/calendar/connections/google/sync`,
    {
      method: "POST",
      headers: requestHeaders(access),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isGoogleCalendarConnection(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function disconnectGoogleCalendar(
  baseUrl: string,
  access: string,
  expectedVersion: number,
): Promise<void> {
  if (!Number.isSafeInteger(expectedVersion) || expectedVersion <= 0) {
    throw new PlanningRequestError("invalid");
  }
  const url = new URL(
    `${normalizeBaseUrl(baseUrl)}/v1/calendar/connections/google`,
  );
  url.searchParams.set("expectedVersion", String(expectedVersion));
  const response = await fetch(url.toString(), {
    method: "DELETE",
    headers: requestHeaders(access),
  });
  if (!response.ok) {
    throw errorFromStatus(response.status);
  }
}

function requestHeaders(access: string, json = false): Record<string, string> {
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

function isGoogleCalendarConnection(
  value: unknown,
): value is GoogleCalendarConnection {
  if (!isRecord(value) || !isConnectionStatus(value.status)) return false;
  return (
    typeof value.available === "boolean" &&
    (typeof value.email === "string" || value.email === null) &&
    Array.isArray(value.grantedScopes) &&
    value.grantedScopes.every((scope) => typeof scope === "string") &&
    (typeof value.lastSuccessfulSyncAt === "string" ||
      value.lastSuccessfulSyncAt === null) &&
    (typeof value.lastErrorCode === "string" || value.lastErrorCode === null) &&
    typeof value.reauthRequired === "boolean" &&
    (typeof value.version === "number" || value.version === null)
  );
}

function isGoogleCalendarAuthorization(
  value: unknown,
): value is GoogleCalendarAuthorization {
  return (
    isRecord(value) &&
    typeof value.authorizationId === "string" &&
    typeof value.authorizationUrl === "string" &&
    typeof value.expiresAt === "string"
  );
}

function isConnectionStatus(
  value: unknown,
): value is GoogleCalendarConnectionStatus {
  return (
    typeof value === "string" &&
    [
      "not_connected",
      "connecting",
      "active",
      "reauth_required",
      "revoking",
      "revoked",
      "error",
    ].includes(value)
  );
}

function isTrustedGoogleAuthorizationUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname === "accounts.google.com";
  } catch {
    return false;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
