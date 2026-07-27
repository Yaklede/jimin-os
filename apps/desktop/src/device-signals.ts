import { invoke, isTauri } from "@tauri-apps/api/core";

import { mobileCapabilitySnapshot } from "./mobile-capabilities";

export type CallLogPermission =
  "not_determined" | "granted" | "denied" | "unavailable";

export type NativeCallLogPermission = {
  status: CallLogPermission;
  canRequest: boolean;
  platformVersion: string;
};

export type NativeMissedCall = {
  sourceId: string;
  occurredAtEpochMillis: number;
  callerName?: string;
  phoneNumber?: string;
};

export type NativeMissedCallSnapshot = {
  calls: NativeMissedCall[];
  platformVersion: string;
};

export type DeviceSignalRuntime = {
  tauri: boolean;
  userAgent: string;
  invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
};

export function deviceSignalsSupported(
  runtime: DeviceSignalRuntime = currentRuntime(),
): boolean {
  return mobileCapabilitySnapshot(runtime).missedCallHistory;
}

export async function getCallLogPermission(
  runtime: DeviceSignalRuntime = currentRuntime(),
): Promise<NativeCallLogPermission> {
  if (!deviceSignalsSupported(runtime)) {
    return {
      status: "unavailable",
      canRequest: false,
      platformVersion: "",
    };
  }
  return parsePermission(
    await runtime.invoke("plugin:device-signals|permissionStatus"),
  );
}

export async function requestCallLogPermission(
  runtime: DeviceSignalRuntime = currentRuntime(),
): Promise<NativeCallLogPermission> {
  if (!deviceSignalsSupported(runtime)) {
    return {
      status: "unavailable",
      canRequest: false,
      platformVersion: "",
    };
  }
  return parsePermission(
    await runtime.invoke("plugin:device-signals|requestPermission"),
  );
}

export async function openCallLogSettings(
  runtime: DeviceSignalRuntime = currentRuntime(),
): Promise<void> {
  if (!deviceSignalsSupported(runtime)) {
    throw new Error("CALL_LOG_UNAVAILABLE");
  }
  await runtime.invoke("plugin:device-signals|openSettings");
}

export async function readNativeMissedCalls(
  sinceEpochMillis: number,
  limit = 200,
  runtime: DeviceSignalRuntime = currentRuntime(),
): Promise<NativeMissedCallSnapshot> {
  if (
    !deviceSignalsSupported(runtime) ||
    !Number.isSafeInteger(sinceEpochMillis) ||
    sinceEpochMillis <= 0 ||
    !Number.isInteger(limit) ||
    limit < 1 ||
    limit > 200
  ) {
    throw new Error("CALL_LOG_UNAVAILABLE");
  }
  const result = await runtime.invoke("plugin:device-signals|missedCalls", {
    sinceEpochMillis,
    limit,
  });
  if (!isRecord(result) || !Array.isArray(result.calls)) {
    throw new Error("CALL_LOG_INVALID_RESULT");
  }
  const calls = result.calls.map(parseMissedCall);
  return {
    calls,
    platformVersion:
      typeof result.platformVersion === "string" ? result.platformVersion : "",
  };
}

function parsePermission(value: unknown): NativeCallLogPermission {
  if (
    !isRecord(value) ||
    !isCallLogPermission(value.status) ||
    typeof value.canRequest !== "boolean" ||
    typeof value.platformVersion !== "string"
  ) {
    throw new Error("CALL_LOG_INVALID_RESULT");
  }
  return {
    status: value.status,
    canRequest: value.canRequest,
    platformVersion: value.platformVersion,
  };
}

function parseMissedCall(value: unknown): NativeMissedCall {
  if (
    !isRecord(value) ||
    typeof value.sourceId !== "string" ||
    value.sourceId.length === 0 ||
    typeof value.occurredAtEpochMillis !== "number" ||
    !Number.isSafeInteger(value.occurredAtEpochMillis) ||
    (value.callerName !== undefined && typeof value.callerName !== "string") ||
    (value.phoneNumber !== undefined && typeof value.phoneNumber !== "string")
  ) {
    throw new Error("CALL_LOG_INVALID_RESULT");
  }
  return {
    sourceId: value.sourceId,
    occurredAtEpochMillis: value.occurredAtEpochMillis,
    callerName: value.callerName as string | undefined,
    phoneNumber: value.phoneNumber as string | undefined,
  };
}

function currentRuntime(): DeviceSignalRuntime {
  return {
    tauri: isTauri(),
    userAgent: typeof navigator === "undefined" ? "" : navigator.userAgent,
    invoke: (command: string, args?: Record<string, unknown>) =>
      invoke(command, args),
  };
}

function isCallLogPermission(value: unknown): value is CallLogPermission {
  return (
    value === "not_determined" ||
    value === "granted" ||
    value === "denied" ||
    value === "unavailable"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
