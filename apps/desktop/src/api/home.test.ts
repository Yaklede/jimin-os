import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchHomeSnapshot } from "./home";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("home snapshot API", () => {
  it("loads the daily server snapshot with the requested local-day range", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        '{"schedule":[],"tasks":[],"recommendations":[],"weeklyReports":[]}',
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchHomeSnapshot(
        "https://jimin-os.example/",
        "session-access",
        new Date("2026-07-12T00:00:00+09:00"),
        new Date("2026-07-13T00:00:00+09:00"),
      ),
    ).resolves.toEqual({
      schedule: [],
      tasks: [],
      dueTasks: [],
      inflow: [],
      recentInflow: [],
      recommendations: [],
      weeklyReports: [],
    });

    const [url, options] = fetchMock.mock.calls[0] ?? [];
    expect(url).toContain("https://jimin-os.example/v1/home?");
    expect(String(url)).toContain("from=2026-07-11T15%3A00%3A00.000Z");
    expect(options?.headers).toMatchObject({
      Authorization: "Bearer session-access",
    });
  });

  it("keeps weekly operation reports in the home snapshot", async () => {
    const weeklyReport = {
      workspaceId: "019f0000-0000-7000-8000-000000000001",
      periodStart: "2026-07-20T00:00:00+09:00",
      periodEnd: "2026-07-24T22:00:00+09:00",
      createdTaskCount: 7,
      completedTaskCount: 4,
      backlogStartCount: 10,
      backlogEndCount: 13,
      backlogDelta: 3,
      overdueTaskCount: 1,
      staleTaskCount: 2,
      unassignedTaskCount: 0,
      projects: [],
    };
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(
          JSON.stringify({
            schedule: [],
            tasks: [],
            weeklyReports: [weeklyReport],
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      ),
    );

    const snapshot = await fetchHomeSnapshot(
      "https://jimin-os.example",
      "session-access",
      new Date("2026-07-24T00:00:00+09:00"),
      new Date("2026-07-25T00:00:00+09:00"),
    );

    expect(snapshot.weeklyReports).toEqual([weeklyReport]);
  });

  it("loads weekly reports from existing endpoints during a rolling server update", async () => {
    const workspaceId = "019f0000-0000-7000-8000-000000000001";
    const weeklyReport = {
      workspaceId,
      periodStart: "2026-07-20T00:00:00+09:00",
      periodEnd: "2026-07-24T22:00:00+09:00",
      createdTaskCount: 3,
      completedTaskCount: 2,
      backlogStartCount: 1,
      backlogEndCount: 2,
      backlogDelta: 1,
      overdueTaskCount: 0,
      staleTaskCount: 1,
      unassignedTaskCount: 0,
      projects: [],
    };
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        new Response('{"schedule":[],"tasks":[]}', {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            items: [
              {
                id: workspaceId,
                scope: "company",
                name: "회사",
                version: 1,
              },
            ],
            nextCursor: null,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify(weeklyReport), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    const snapshot = await fetchHomeSnapshot(
      "https://jimin-os.example",
      "session-access",
      new Date("2026-07-24T00:00:00+09:00"),
      new Date("2026-07-25T00:00:00+09:00"),
    );

    expect(snapshot.weeklyReports).toEqual([weeklyReport]);
    expect(String(fetchMock.mock.calls[1]?.[0])).toBe(
      "https://jimin-os.example/v1/workspaces",
    );
    expect(String(fetchMock.mock.calls[2]?.[0])).toContain(
      `/v1/reports/weekly?workspaceId=${workspaceId}`,
    );
  });
});
