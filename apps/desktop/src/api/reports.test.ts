import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createProjectWeeklyReport,
  fetchProjectReports,
  finalizeReport,
  type Report,
  updateReport,
} from "./reports";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("project report API", () => {
  const report: Report = {
    id: "019f68cb-9400-7000-8000-000000000041",
    workspaceId: "019f68cb-9400-7000-8000-000000000042",
    projectId: "019f68cb-9400-7000-8000-000000000043",
    reportType: "project_weekly",
    title: "비스킷링크 주간 운영 보고서",
    periodStart: "2026-07-27T00:00:00Z",
    periodEnd: "2026-08-03T00:00:00Z",
    status: "draft",
    currentVersion: 1,
    content: {
      kind: "project_weekly",
      period: {
        start: "2026-07-27T00:00:00Z",
        end: "2026-08-03T00:00:00Z",
      },
      summary: "이번 주 운영 흐름입니다.",
      metrics: [{ key: "created", label: "새로 들어온 일", value: 3 }],
      focus: ["기한이 지난 일을 확인하세요."],
      evidence: [
        {
          type: "weekly_metrics",
          workspaceId: "019f68cb-9400-7000-8000-000000000042",
          projectId: "019f68cb-9400-7000-8000-000000000043",
        },
      ],
    },
    generatedAt: "2026-08-03T00:00:00Z",
    finalizedAt: null,
    createdAt: "2026-08-03T00:00:00Z",
    updatedAt: "2026-08-03T00:00:00Z",
    version: 1,
  };

  it("loads reports scoped to a workspace and project", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ items: [report], nextCursor: null }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchProjectReports(
        "https://jimin-os.example/",
        "access",
        report.workspaceId,
        report.projectId,
      ),
    ).resolves.toEqual([report]);

    const [url, options] = fetchMock.mock.calls[0] ?? [];
    expect(String(url)).toContain("/v1/reports?");
    expect(String(url)).toContain(`workspaceId=${report.workspaceId}`);
    expect(String(url)).toContain(`projectId=${report.projectId}`);
    expect(options?.headers).toMatchObject({ Authorization: "Bearer access" });
  });

  it("sends report edits with the optimistic version", async () => {
    const updated = { ...report, version: 2, currentVersion: 2 };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(updated), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      updateReport("https://jimin-os.example", "access", report, {
        ...report.content,
        summary: "수정한 요약입니다.",
      }),
    ).resolves.toEqual(updated);

    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual(
      expect.objectContaining({ expectedVersion: 1 }),
    );
  });

  it("rejects malformed report content from a server response", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn<typeof fetch>()
        .mockResolvedValue(
          new Response(
            JSON.stringify({ ...report, content: { summary: "not enough" } }),
            { status: 200 },
          ),
        ),
    );

    await expect(
      createProjectWeeklyReport(
        "https://jimin-os.example",
        "access",
        report.workspaceId,
        report.projectId,
      ),
    ).rejects.toMatchObject({ code: "unavailable" });
  });

  it("finalizes a report through the versioned endpoint", async () => {
    const finalized = {
      ...report,
      status: "finalized" as const,
      finalizedAt: "2026-08-03T01:00:00Z",
      version: 2,
    };
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(JSON.stringify(finalized), { status: 200 }),
      );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      finalizeReport("https://jimin-os.example", "access", report),
    ).resolves.toEqual(finalized);
    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
      expectedVersion: 1,
    });
  });
});
