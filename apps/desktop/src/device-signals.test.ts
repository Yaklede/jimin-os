import { describe, expect, it, vi } from "vitest";

import {
  getCallLogPermission,
  readNativeMissedCalls,
  requestCallLogPermission,
  type DeviceSignalRuntime,
} from "./device-signals";

function runtime(invoke: DeviceSignalRuntime["invoke"]): DeviceSignalRuntime {
  return {
    tauri: true,
    userAgent: "Mozilla/5.0 (Linux; Android 16)",
    invoke,
  };
}

describe("Android device signal boundary", () => {
  it("reads and requests the sensitive permission through explicit commands", async () => {
    const invoke = vi.fn(async () => ({
      status: "granted",
      canRequest: false,
      platformVersion: "16",
    }));
    const testRuntime = runtime(invoke);

    await expect(getCallLogPermission(testRuntime)).resolves.toMatchObject({
      status: "granted",
    });
    await requestCallLogPermission(testRuntime);
    expect(invoke).toHaveBeenNthCalledWith(
      1,
      "plugin:device-signals|permissionStatus",
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "plugin:device-signals|requestPermission",
    );
  });

  it("normalizes a bounded native missed-call snapshot", async () => {
    const invoke = vi.fn(async () => ({
      calls: [
        {
          sourceId: "42",
          occurredAtEpochMillis: 1_785_100_000_000,
          callerName: "홍길동",
          phoneNumber: "010-0000-0000",
        },
      ],
      platformVersion: "16",
    }));

    await expect(
      readNativeMissedCalls(1_785_000_000_000, 50, runtime(invoke)),
    ).resolves.toMatchObject({
      calls: [{ sourceId: "42", callerName: "홍길동" }],
    });
    expect(invoke).toHaveBeenCalledWith("plugin:device-signals|missedCalls", {
      sinceEpochMillis: 1_785_000_000_000,
      limit: 50,
    });
  });

  it("does not expose the plugin from a desktop or browser runtime", async () => {
    const invoke = vi.fn(async () => undefined);
    const desktop: DeviceSignalRuntime = {
      tauri: true,
      userAgent: "Mozilla/5.0 Macintosh",
      invoke,
    };

    await expect(getCallLogPermission(desktop)).resolves.toMatchObject({
      status: "unavailable",
    });
    expect(invoke).not.toHaveBeenCalled();
  });
});
