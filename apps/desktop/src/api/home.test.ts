import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchHomeSnapshot } from "./home";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("home snapshot API", () => {
  it("loads the daily server snapshot with the requested local-day range", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response('{"schedule":[],"tasks":[],"recommendations":[]}', {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
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
      recommendations: [],
    });

    const [url, options] = fetchMock.mock.calls[0] ?? [];
    expect(url).toContain("https://jimin-os.example/v1/home?");
    expect(String(url)).toContain("from=2026-07-11T15%3A00%3A00.000Z");
    expect(options?.headers).toMatchObject({
      Authorization: "Bearer session-access",
    });
  });
});
