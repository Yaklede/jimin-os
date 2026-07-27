import { PlanningRequestError } from "./planning";

export type CallLogPermission =
  "not_determined" | "granted" | "denied" | "unavailable";

export interface DeviceSignalState {
  deviceId: string;
  deviceName: string;
  callLogPermission: CallLogPermission;
  platformVersion: string | null;
  appVersion: string | null;
  lastSyncedAt: string | null;
}

export interface MissedCallUpload {
  sourceId: string;
  occurredAt: string;
  callerName?: string;
  phoneNumber?: string;
}

export interface DeviceSignalSyncResponse {
  insertedCount: number;
  state: DeviceSignalState;
}

export async function fetchDeviceSignalStates(
  baseUrl: string,
  access: string,
): Promise<DeviceSignalState[]> {
  const response = await request(baseUrl, access, "/v1/device-signals/status");
  if (!isRecord(response) || !Array.isArray(response.items)) {
    throw new PlanningRequestError("unavailable");
  }
  return response.items.map(parseState);
}

export async function synchronizeMissedCalls(
  baseUrl: string,
  access: string,
  input: {
    permission: CallLogPermission;
    platformVersion?: string;
    appVersion?: string;
    calls: MissedCallUpload[];
  },
): Promise<DeviceSignalSyncResponse> {
  const response = await request(
    baseUrl,
    access,
    "/v1/device-signals/missed-calls",
    {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    },
  );
  if (
    !isRecord(response) ||
    typeof response.insertedCount !== "number" ||
    !Number.isSafeInteger(response.insertedCount)
  ) {
    throw new PlanningRequestError("unavailable");
  }
  return {
    insertedCount: response.insertedCount,
    state: parseState(response.state),
  };
}

async function request(
  baseUrl: string,
  access: string,
  path: string,
  init: RequestInit = {},
): Promise<unknown> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    ...init,
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${access}`,
      ...init.headers,
    },
  });
  const body: unknown = await response.json().catch(() => undefined);
  if (!response.ok) throw errorFromStatus(response.status);
  return body;
}

function parseState(value: unknown): DeviceSignalState {
  if (
    !isRecord(value) ||
    typeof value.deviceId !== "string" ||
    typeof value.deviceName !== "string" ||
    !isPermission(value.callLogPermission) ||
    !optionalString(value.platformVersion) ||
    !optionalString(value.appVersion) ||
    !optionalString(value.lastSyncedAt)
  ) {
    throw new PlanningRequestError("unavailable");
  }
  return value as unknown as DeviceSignalState;
}

function isPermission(value: unknown): value is CallLogPermission {
  return (
    value === "not_determined" ||
    value === "granted" ||
    value === "denied" ||
    value === "unavailable"
  );
}

function optionalString(value: unknown): boolean {
  return value === null || typeof value === "string";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
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
