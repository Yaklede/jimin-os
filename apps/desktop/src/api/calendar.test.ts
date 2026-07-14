import { afterEach, describe, expect, it, vi } from "vitest";

import {
  fetchGoogleCalendarConnection,
  startGoogleCalendarAuthorization,
  synchronizeGoogleCalendar,
} from "./calendar";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const connection = {
  available: true,
  status: "active",
  email: "owner@example.com",
  grantedScopes: ["calendar.readonly"],
  lastSuccessfulSyncAt: "2026-07-14T05:00:00Z",
  lastErrorCode: null,
  reauthRequired: false,
  version: 2,
};

describe("Google Calendar client", () => {
  it("loads the server-owned connection state", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(connection), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGoogleCalendarConnection("https://jimin-os.example/", "access"),
    ).resolves.toEqual(connection);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/calendar/connections/google",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer access" }),
      }),
    );
  });

  it("starts a platform-bound authorization only for the Google consent host", async () => {
    const authorization = {
      authorizationId: "019f68cb-9400-7000-8000-000000000001",
      authorizationUrl:
        "https://accounts.google.com/o/oauth2/v2/auth?state=safe",
      expiresAt: "2026-07-14T05:10:00Z",
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(authorization), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      startGoogleCalendarAuthorization(
        "https://jimin-os.example",
        "access",
        "Mozilla/5.0 (Linux; Android 16)",
      ),
    ).resolves.toEqual(authorization);
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      clientKind: "android",
    });
  });

  it("rejects an authorization URL outside the expected Google host", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(
          JSON.stringify({
            authorizationId: "019f68cb-9400-7000-8000-000000000001",
            authorizationUrl: "https://example.com/not-google",
            expiresAt: "2026-07-14T05:10:00Z",
          }),
          { status: 201, headers: { "Content-Type": "application/json" } },
        ),
      ),
    );

    await expect(
      startGoogleCalendarAuthorization("https://jimin-os.example", "access"),
    ).rejects.toMatchObject({ code: "unavailable" });
  });

  it("requests a server-side calendar synchronization", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(connection), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      synchronizeGoogleCalendar("https://jimin-os.example", "access"),
    ).resolves.toEqual(connection);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/calendar/connections/google/sync",
      expect.objectContaining({ method: "POST" }),
    );
  });
});
