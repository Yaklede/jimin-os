import { invoke, isTauri } from "@tauri-apps/api/core";

import type { SessionTokens } from "./api/planning";

const previewSessionKey = "jimin-os-dev-session";

export interface StoredDeviceSession {
  apiBaseUrl: string;
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

function parseSession(value: string): StoredDeviceSession | undefined {
  try {
    const parsed: unknown = JSON.parse(value);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      typeof (parsed as { apiBaseUrl?: unknown }).apiBaseUrl !== "string" ||
      typeof (parsed as { tokens?: { accessToken?: unknown } }).tokens
        ?.accessToken !== "string" ||
      typeof (parsed as { tokens?: { refreshToken?: unknown } }).tokens
        ?.refreshToken !== "string"
    ) {
      return undefined;
    }

    return parsed as StoredDeviceSession;
  } catch {
    return undefined;
  }
}
