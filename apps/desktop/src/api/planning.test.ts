import { afterEach, describe, expect, it, vi } from "vitest";

import {
  PlanningRequestError,
  bootstrapTrustedNetworkSession,
  clientPlatformForUserAgent,
  deleteScheduleEntry,
  fetchPlanning,
  refreshDeviceSession,
  updateScheduleEntry,
  updateTask,
} from "./planning";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("device session client", () => {
  it("starts a private-server session without an interactive pairing step", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"accessToken":"a","refreshToken":"r","user":{},"device":{},"syncCursor":"0"}',
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("navigator", {
      platform: "Linux armv8l",
      userAgent: "Mozilla/5.0 (Linux; Android 16; Pixel)",
    });

    await expect(
      bootstrapTrustedNetworkSession(
        "http://127.0.0.1:8080",
        "Jimin OS",
        "019f68cb-9400-7000-8000-000000000000",
      ),
    ).resolves.toEqual({
      accessToken: "a",
      refreshToken: "r",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:8080/v1/access/session",
      expect.objectContaining({ method: "POST" }),
    );
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toMatchObject({
      installationId: "019f68cb-9400-7000-8000-000000000000",
      platform: "android",
      name: "Jimin OS",
    });
  });

  it("does not send an invalid installation identity to the personal server", async () => {
    const fetchMock = vi.fn<typeof fetch>();
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      bootstrapTrustedNetworkSession(
        "https://jimin-os.example",
        "Jimin OS",
        "550e8400-e29b-41d4-a716-446655440000",
      ),
    ).rejects.toMatchObject({
      code: "invalid",
    } satisfies Partial<PlanningRequestError>);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("rotates a device session through the server refresh endpoint", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"accessToken":"a","refreshToken":"r","user":{},"device":{},"syncCursor":"0"}',
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);

    const session = await refreshDeviceSession(
      "https://jimin-os.example/",
      "previous-refresh",
    );

    expect(session.accessToken).toBe("a");
    expect(session.refreshToken).toBe("r");
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

  it("uses the current client platform for the private-server session", () => {
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
});

describe("task client", () => {
  it("loads open and completed tasks as separate planning collections", async () => {
    const completedTask = {
      id: "019f68cb-9400-7000-8000-000000000012",
      projectId: null,
      title: "배포 완료",
      notes: null,
      status: "completed",
      priority: 2,
      dueAt: null,
      completedAt: "2026-07-14T01:00:00Z",
      version: 2,
    };
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        new Response('{"items":[],"nextCursor":null}', {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response('{"items":[],"nextCursor":null}', {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ items: [completedTask], nextCursor: null }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("window", { location: { origin: "https://desktop.local" } });

    await expect(
      fetchPlanning(
        "https://jimin-os.example/",
        "access",
        new Date("2026-07-14T00:00:00Z"),
        new Date("2026-07-15T00:00:00Z"),
      ),
    ).resolves.toEqual({
      schedule: [],
      tasks: [],
      completedTasks: [completedTask],
    });
    expect(String(fetchMock.mock.calls[2]?.[0])).toContain("status=completed");
  });

  it("sends a version-checked task update", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "019f68cb-9400-7000-8000-000000000010",
          projectId: "019f68cb-9400-7000-8000-000000000011",
          title: "계약서 검토",
          notes: "수정본 확인",
          status: "open",
          priority: 3,
          dueAt: null,
          completedAt: null,
          version: 5,
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await updateTask(
      "https://jimin-os.example/",
      "access",
      {
        id: "019f68cb-9400-7000-8000-000000000010",
        projectId: "019f68cb-9400-7000-8000-000000000011",
        title: "계약서 검토",
        notes: null,
        status: "open",
        priority: 1,
        dueAt: null,
        completedAt: null,
        version: 4,
      },
      {
        title: "계약서 검토",
        notes: "수정본 확인",
        status: "open",
        priority: 3,
      },
    );

    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/tasks/019f68cb-9400-7000-8000-000000000010",
      expect.objectContaining({ method: "PUT" }),
    );
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toMatchObject({
      projectId: "019f68cb-9400-7000-8000-000000000011",
      title: "계약서 검토",
      notes: "수정본 확인",
      status: "open",
      priority: 3,
      expectedVersion: 4,
    });
  });
});

describe("schedule client", () => {
  it("sends a version-checked manual schedule update", async () => {
    const updated = {
      id: "019f68cb-9400-7000-8000-000000000020",
      title: "치과 방문",
      notes: "접수 10분 전",
      startsAt: "2026-07-14T08:00:00.000Z",
      endsAt: "2026-07-14T09:00:00.000Z",
      timeZone: "Asia/Seoul",
      status: "confirmed" as const,
      source: "manual" as const,
      editable: true,
      version: 3,
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(updated), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await updateScheduleEntry(
      "https://jimin-os.example/",
      "access",
      { ...updated, version: 2 },
      {
        title: updated.title,
        notes: updated.notes,
        startsAt: updated.startsAt,
        endsAt: updated.endsAt,
      },
    );

    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/schedule-entries/019f68cb-9400-7000-8000-000000000020",
      expect.objectContaining({ method: "PUT" }),
    );
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toMatchObject({
      title: "치과 방문",
      notes: "접수 10분 전",
      startsAt: "2026-07-14T08:00:00.000Z",
      endsAt: "2026-07-14T09:00:00.000Z",
      expectedVersion: 2,
    });
  });

  it("sends a version-checked manual schedule deletion", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await deleteScheduleEntry("https://jimin-os.example/", "access", {
      id: "019f68cb-9400-7000-8000-000000000020",
      title: "치과 방문",
      notes: null,
      startsAt: "2026-07-14T08:00:00.000Z",
      endsAt: "2026-07-14T09:00:00.000Z",
      timeZone: "Asia/Seoul",
      status: "confirmed",
      source: "manual",
      editable: true,
      version: 3,
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/schedule-entries/019f68cb-9400-7000-8000-000000000020",
      expect.objectContaining({ method: "DELETE" }),
    );
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({ expectedVersion: 3 });
  });
});
