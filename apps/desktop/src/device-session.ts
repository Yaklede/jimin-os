import { invoke, isTauri } from "@tauri-apps/api/core";

import type { SessionTokens } from "./api/planning";
import { createUuidV7, isUuidV7 } from "./uuid";

const previewSessionKey = "jimin-os-dev-session";
const previewInstallationKey = "jimin-os-dev-installation";

export interface StoredDeviceSession {
  tokens: SessionTokens;
}

export async function readDeviceSession(): Promise<
  StoredDeviceSession | undefined
> {
  const raw = isTauri()
    ? await invoke<string | undefined>("read_device_session")
    : (sessionStorage.getItem(previewSessionKey) ?? undefined);

  if (!raw) return undefined;

  const session = parseSession(raw);
  if (session) return session;

  await clearDeviceSession();
  return undefined;
}

export async function saveDeviceSession(
  session: StoredDeviceSession,
): Promise<void> {
  const value = JSON.stringify(session);
  if (isTauri()) {
    await invoke("save_device_session", { value });
    return;
  }
  sessionStorage.setItem(previewSessionKey, value);
}

export async function clearDeviceSession(): Promise<void> {
  if (isTauri()) {
    await invoke("clear_device_session");
    return;
  }
  sessionStorage.removeItem(previewSessionKey);
}

export async function readOrCreateInstallationId(): Promise<string> {
  const value = isTauri()
    ? await invoke<string>("read_or_create_installation_id")
    : readOrCreatePreviewInstallationId();
  if (!isUuidV7(value)) {
    throw new Error("The device identity is invalid.");
  }
  return value;
}

function readOrCreatePreviewInstallationId(): string {
  const existing = localStorage.getItem(previewInstallationKey);
  if (existing && isUuidV7(existing)) return existing;
  const installationId = createUuidV7();
  localStorage.setItem(previewInstallationKey, installationId);
  return installationId;
}

function parseSession(value: string): StoredDeviceSession | undefined {
  try {
    const parsed: unknown = JSON.parse(value);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      typeof (parsed as { tokens?: { accessToken?: unknown } }).tokens
        ?.accessToken !== "string" ||
      typeof (parsed as { tokens?: { refreshToken?: unknown } }).tokens
        ?.refreshToken !== "string"
    ) {
      return undefined;
    }

    return {
      tokens: (parsed as { tokens: SessionTokens }).tokens,
    };
  } catch {
    return undefined;
  }
}
