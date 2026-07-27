import { afterEach, describe, expect, it, vi } from "vitest";

import {
  fetchDeviceSignalStates,
  synchronizeMissedCalls,
} from "./deviceSignals";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("device signal API", () => {
  it("loads safe Android connection metadata", async () => {
    const fetchMock = vi.fn(
      async (_input: RequestInfo | URL, _init?: RequestInit) =>
        new Response(
          JSON.stringify({
            items: [
              {
                deviceId: "device",
                deviceName: "Galaxy",
                callLogPermission: "granted",
                platformVersion: "16",
                appVersion: "0.1.0",
                lastSyncedAt: "2026-07-27T01:00:00Z",
              },
            ],
          }),
          { status: 200 },
        ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchDeviceSignalStates("https://os.example/", "access"),
    ).resolves.toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://os.example/v1/device-signals/status",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer access" }),
      }),
    );
  });

  it("uploads only the normalized missed-call contract", async () => {
    const fetchMock = vi.fn(
      async (_input: RequestInfo | URL, _init?: RequestInit) =>
        new Response(
          JSON.stringify({
            insertedCount: 1,
            state: {
              deviceId: "device",
              deviceName: "Galaxy",
              callLogPermission: "granted",
              platformVersion: "16",
              appVersion: "0.1.0",
              lastSyncedAt: "2026-07-27T01:00:00Z",
            },
          }),
          { status: 200 },
        ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await synchronizeMissedCalls("https://os.example", "access", {
      permission: "granted",
      platformVersion: "16",
      calls: [
        {
          sourceId: "42",
          occurredAt: "2026-07-27T01:00:00.000Z",
          callerName: "홍길동",
        },
      ],
    });
    const [, init] = fetchMock.mock.calls[0] ?? [];
    expect(JSON.parse(String(init?.body))).toMatchObject({
      permission: "granted",
      calls: [{ sourceId: "42", callerName: "홍길동" }],
    });
  });
});
