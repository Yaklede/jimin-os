import { afterEach, describe, expect, it, vi } from "vitest";

import {
  PlanningRequestError,
  bootstrapLocalPhoneTestSession,
  clientPlatformForUserAgent,
  exchangePairingCode,
  isLocalPhoneTest,
  pairingTokenFromScannedQr,
  pairingTokenFromValue,
  refreshDeviceSession,
} from "./planning";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("refreshDeviceSession", () => {
  it("enables automatic setup only for the explicit local phone test build", () => {
    expect(isLocalPhoneTest({ VITE_LOCAL_PHONE_TEST: "1" })).toBe(true);
    expect(isLocalPhoneTest({ VITE_LOCAL_PHONE_TEST: "true" })).toBe(false);
    expect(isLocalPhoneTest({})).toBe(false);
  });

  it("bootstraps a debug device through the local test-only route", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"accessToken":"access-session","refreshToken":"refresh-session","user":{},"device":{},"syncCursor":"0"}',
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("navigator", {
      platform: "Linux armv8l",
      userAgent: "Mozilla/5.0 (Linux; Android 16; Pixel)",
    });

    await expect(
      bootstrapLocalPhoneTestSession(
        "http://127.0.0.1:8080",
        "개발용 Android",
        "019f68cb-9400-7000-8000-000000000000",
      ),
    ).resolves.toEqual({
      ["accessToken"]: "access-session",
      ["refreshToken"]: "refresh-session",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:8080/v1/testing/local-phone-bootstrap",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("rotates a device session through the server refresh endpoint", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"accessToken":"access-session","refreshToken":"refresh-session","user":{},"device":{},"syncCursor":"0"}',
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);

    const session = await refreshDeviceSession(
      "https://jimin-os.example/",
      "previous-refresh",
    );

    expect(session.accessToken).toBe("access-session");
    expect(session.refreshToken).toBe("refresh-session");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/auth/refresh",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("uses a classified error when the refresh session cannot be used", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn<typeof fetch>()
        .mockResolvedValue(new Response(null, { status: 401 })),
    );

    await expect(
      refreshDeviceSession("/server", "expired-refresh"),
    ).rejects.toMatchObject({
      code: "unauthorized",
    } satisfies Partial<PlanningRequestError>);
  });

  it("accepts the complete pairing URI produced for a QR connection", () => {
    const code = "jp_019f68cb-9400-7000-8000-000000000000.example";
    const pairingUri = new URL("jimin-os://pair");
    pairingUri.searchParams.set("token", code);

    expect(pairingTokenFromValue(pairingUri.toString())).toBe(code);
    expect(pairingTokenFromScannedQr(pairingUri.toString())).toBe(code);
  });

  it("rejects a non-pairing QR value before it reaches the server", () => {
    expect(pairingTokenFromScannedQr("https://example.com/invitation")).toBe(
      "",
    );
  });

  it("uses the client platform for device registration", () => {
    expect(
      clientPlatformForUserAgent(
        "Mozilla/5.0 (Linux; Android 16; Pixel) AppleWebKit/537.36",
      ),
    ).toBe("android");
    expect(
      clientPlatformForUserAgent("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0)"),
    ).toBe("ios");
    expect(
      clientPlatformForUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X)"),
    ).toBe("macos");
  });

  it("sends the stable version-seven installation ID when a device exchanges a code", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"accessToken":"access-session","refreshToken":"refresh-session","user":{},"device":{},"syncCursor":"0"}',
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("navigator", {
      platform: "Linux armv8l",
      userAgent: "Mozilla/5.0 (Linux; Android 16; Pixel)",
    });
    const code = "jp_019f68cb-9400-7000-8000-000000000000.example";
    const installationId = "019f68cb-9400-7000-8000-000000000000";
    const pairingUri = new URL("jimin-os://pair");
    pairingUri.searchParams.set("token", code);

    await exchangePairingCode(
      "https://jimin-os.example",
      pairingUri.toString(),
      "내 기기",
      installationId,
    );

    const request = fetchMock.mock.calls[0]?.[1];
    const body = JSON.parse(String(request?.body));
    expect(body.pairingToken).toBe(code);
    expect(body.device.platform).toBe("android");
    expect(body.device.installationId).toBe(installationId);
  });

  it("rejects a malformed installation ID before sending a pairing request", async () => {
    const fetchMock = vi.fn<typeof fetch>();
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      exchangePairingCode(
        "https://jimin-os.example",
        "pairing-code",
        "내 기기",
        "550e8400-e29b-41d4-a716-446655440000",
      ),
    ).rejects.toMatchObject({
      code: "invalid",
    } satisfies Partial<PlanningRequestError>);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
