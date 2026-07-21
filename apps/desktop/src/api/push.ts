import { PlanningRequestError } from "./planning";

export interface PushRegistration {
  enabled: boolean;
  provider: "fcm";
  lastSeenAt: string | null;
  lastDeliveredAt: string | null;
  lastErrorCode: string | null;
}

const fcmRegistrationField = "token";

export async function fetchPushRegistration(
  baseUrl: string,
  access: string,
): Promise<PushRegistration> {
  return requestRegistration(baseUrl, access, { method: "GET" });
}

export async function registerFcmToken(
  baseUrl: string,
  access: string,
  registrationHandle: string,
): Promise<PushRegistration> {
  if (!validFcmToken(registrationHandle)) {
    throw new PlanningRequestError("invalid");
  }
  return requestRegistration(baseUrl, access, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      provider: "fcm",
      [fcmRegistrationField]: registrationHandle,
    }),
  });
}

export async function disablePushRegistration(
  baseUrl: string,
  access: string,
): Promise<void> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/push/registration`,
    {
      method: "DELETE",
      headers: { Authorization: `Bearer ${access}` },
    },
  );
  if (!response.ok) throw errorFromStatus(response.status);
}

async function requestRegistration(
  baseUrl: string,
  access: string,
  init: RequestInit,
): Promise<PushRegistration> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/push/registration`,
    {
      ...init,
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
        ...init.headers,
      },
    },
  );
  const body: unknown = await response.json().catch(() => undefined);
  if (!response.ok) throw errorFromStatus(response.status);
  if (!isPushRegistration(body)) throw new PlanningRequestError("unavailable");
  return body;
}

function isPushRegistration(value: unknown): value is PushRegistration {
  if (typeof value !== "object" || value === null) return false;
  const registration = value as Partial<PushRegistration>;
  return (
    typeof registration.enabled === "boolean" &&
    registration.provider === "fcm" &&
    optionalString(registration.lastSeenAt) &&
    optionalString(registration.lastDeliveredAt) &&
    optionalString(registration.lastErrorCode)
  );
}

function optionalString(value: unknown): boolean {
  return value === null || typeof value === "string";
}

function validFcmToken(value: string): boolean {
  return (
    value.length >= 20 &&
    value.length <= 4096 &&
    value.trim() === value &&
    !/\s/.test(value)
  );
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/+$/, "");
}

function errorFromStatus(status: number): PlanningRequestError {
  if (status === 401) return new PlanningRequestError("unauthorized");
  if (status === 400 || status === 422) {
    return new PlanningRequestError("invalid");
  }
  return new PlanningRequestError("unavailable");
}
