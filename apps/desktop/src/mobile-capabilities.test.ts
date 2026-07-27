import { describe, expect, it, vi } from "vitest";

import {
  cancelNativeVoiceDictation,
  mobileCapabilitySnapshot,
  nativeVoiceDictationSupported,
  startNativeVoiceDictation,
  stopNativeVoiceDictation,
  type MobileCapabilityRuntime,
} from "./mobile-capabilities";

function runtime(
  overrides: Partial<MobileCapabilityRuntime> = {},
): MobileCapabilityRuntime {
  return {
    tauri: true,
    userAgent: "Mozilla/5.0 (Linux; Android 16)",
    invoke: vi.fn(async () => undefined),
    ...overrides,
  };
}

describe("mobile capability boundary", () => {
  it("exposes Android Tauri native capabilities", () => {
    expect(mobileCapabilitySnapshot(runtime())).toEqual({
      platform: "android",
      nativeVoiceDictation: true,
      localNotifications: true,
      nativeBackNavigation: true,
    });
  });

  it("keeps browser and desktop fallback capabilities disabled", () => {
    expect(
      mobileCapabilitySnapshot(
        runtime({ tauri: false, userAgent: "Mozilla/5.0 Chrome" }),
      ),
    ).toEqual({
      platform: "web",
      nativeVoiceDictation: false,
      localNotifications: false,
      nativeBackNavigation: false,
    });
    expect(
      mobileCapabilitySnapshot(
        runtime({ tauri: true, userAgent: "Mozilla/5.0 Macintosh" }),
      ).platform,
    ).toBe("desktop");
  });

  it("normalizes a native dictation response through one contract", async () => {
    const invoke = vi.fn(async () => ({ transcript: "  회의 내용  " }));
    const testRuntime = runtime({ invoke });

    await expect(startNativeVoiceDictation(testRuntime)).resolves.toEqual({
      transcript: "회의 내용",
    });
    expect(invoke).toHaveBeenCalledWith("plugin:voice-recognition|start");
  });

  it("rejects malformed plugin responses and unsupported platforms", async () => {
    await expect(
      startNativeVoiceDictation(
        runtime({ invoke: vi.fn(async () => ({ text: "missing" })) }),
      ),
    ).rejects.toThrow("VOICE_INVALID_RESULT");

    const browserRuntime = runtime({
      tauri: false,
      userAgent: "Mozilla/5.0 Chrome",
    });
    expect(nativeVoiceDictationSupported(browserRuntime)).toBe(false);
    await expect(startNativeVoiceDictation(browserRuntime)).rejects.toThrow(
      "VOICE_UNAVAILABLE",
    );
  });

  it("routes stop and cancel commands and makes unsupported cancel a no-op", async () => {
    const invoke = vi.fn(async () => undefined);
    const testRuntime = runtime({ invoke });

    await stopNativeVoiceDictation(testRuntime);
    await cancelNativeVoiceDictation(testRuntime);
    expect(invoke).toHaveBeenNthCalledWith(1, "plugin:voice-recognition|stop");
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "plugin:voice-recognition|cancel",
    );

    const browserInvoke = vi.fn(async () => undefined);
    await cancelNativeVoiceDictation(
      runtime({
        tauri: false,
        userAgent: "Mozilla/5.0 Chrome",
        invoke: browserInvoke,
      }),
    );
    expect(browserInvoke).not.toHaveBeenCalled();
  });
});
