import { invoke, isTauri } from "@tauri-apps/api/core";

export type MobilePlatform = "android" | "ios" | "desktop" | "web";

export type MobileCapabilitySnapshot = {
  platform: MobilePlatform;
  nativeVoiceDictation: boolean;
  localNotifications: boolean;
  nativeBackNavigation: boolean;
  missedCallHistory: boolean;
};

export type NativeVoiceResult = {
  transcript: string;
};

export type MobileCapabilityRuntime = {
  tauri: boolean;
  userAgent: string;
  invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
};

export function mobileCapabilitySnapshot(
  runtime: MobileCapabilityRuntime = currentRuntime(),
): MobileCapabilitySnapshot {
  const platform = platformFrom(runtime);
  const isAndroidTauri = runtime.tauri && platform === "android";

  return {
    platform,
    nativeVoiceDictation: isAndroidTauri,
    localNotifications: isAndroidTauri,
    nativeBackNavigation: isAndroidTauri,
    missedCallHistory: isAndroidTauri,
  };
}

export function nativeVoiceDictationSupported(
  runtime: MobileCapabilityRuntime = currentRuntime(),
): boolean {
  return mobileCapabilitySnapshot(runtime).nativeVoiceDictation;
}

export async function startNativeVoiceDictation(
  runtime: MobileCapabilityRuntime = currentRuntime(),
): Promise<NativeVoiceResult> {
  assertNativeVoiceDictation(runtime);
  const result = await runtime.invoke("plugin:voice-recognition|start");
  if (
    typeof result !== "object" ||
    result === null ||
    typeof (result as { transcript?: unknown }).transcript !== "string"
  ) {
    throw new Error("VOICE_INVALID_RESULT");
  }

  return {
    transcript: (result as NativeVoiceResult).transcript.trim(),
  };
}

export async function stopNativeVoiceDictation(
  runtime: MobileCapabilityRuntime = currentRuntime(),
): Promise<void> {
  assertNativeVoiceDictation(runtime);
  await runtime.invoke("plugin:voice-recognition|stop");
}

export async function cancelNativeVoiceDictation(
  runtime: MobileCapabilityRuntime = currentRuntime(),
): Promise<void> {
  if (!nativeVoiceDictationSupported(runtime)) return;
  await runtime.invoke("plugin:voice-recognition|cancel");
}

function currentRuntime(): MobileCapabilityRuntime {
  return {
    tauri: isTauri(),
    userAgent: typeof navigator === "undefined" ? "" : navigator.userAgent,
    invoke: (command: string, args?: Record<string, unknown>) =>
      invoke(command, args),
  };
}

function platformFrom(runtime: MobileCapabilityRuntime): MobilePlatform {
  if (/Android/i.test(runtime.userAgent)) return "android";
  if (/iPhone|iPad|iPod/i.test(runtime.userAgent)) return "ios";
  return runtime.tauri ? "desktop" : "web";
}

function assertNativeVoiceDictation(runtime: MobileCapabilityRuntime): void {
  if (!nativeVoiceDictationSupported(runtime)) {
    throw new Error("VOICE_UNAVAILABLE");
  }
}
