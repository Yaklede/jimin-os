import { afterEach, describe, expect, it, vi } from "vitest";

import { PlanningRequestError, refreshDeviceSession } from "./planning";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("refreshDeviceSession", () => {
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
});
