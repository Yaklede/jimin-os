import { afterEach, describe, expect, it, vi } from "vitest";

import { decideRecommendation, refreshWorkBrief } from "./intelligence";
import { type Recommendation } from "./home";

const recommendation: Recommendation = {
  id: "019f68cb-9400-7000-8000-000000000021",
  workspaceId: null,
  projectId: null,
  goalId: null,
  signalId: "019f68cb-9400-7000-8000-000000000022",
  title: "기한이 지난 할 일을 먼저 확인하세요",
  rationale: "‘계약서 검토’의 기한이 지났어요.",
  expectedEffect: "가장 중요한 한 가지에 먼저 집중할 수 있어요.",
  riskSummary: null,
  confidence: 96,
  urgency: 3,
  impact: 3,
  riskLevel: 2,
  effortMinutes: null,
  suggestedActionKind: "review",
  suggestedEntityId: "019f68cb-9400-7000-8000-000000000023",
  status: "pending",
  validUntil: "2026-07-18T00:00:00Z",
  revisitAt: null,
  createdAt: "2026-07-16T00:00:00Z",
  updatedAt: "2026-07-16T00:00:00Z",
  version: 1,
};

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("work intelligence API", () => {
  it("refreshes the structured work brief for the signed-in owner", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ items: [recommendation] }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      refreshWorkBrief("https://jimin-os.example/", "access"),
    ).resolves.toEqual([recommendation]);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/briefs/work/refresh",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({ Authorization: "Bearer access" }),
      }),
    );
  });

  it("defers one recommendation with an idempotent versioned decision", async () => {
    const deferred = {
      ...recommendation,
      status: "deferred" as const,
      revisitAt: "2026-07-16T08:00:00Z",
      version: 2,
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(deferred), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      decideRecommendation(
        "https://jimin-os.example",
        "access",
        recommendation,
        "defer",
        deferred.revisitAt,
      ),
    ).resolves.toEqual(deferred);
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toMatchObject({
      decision: "defer",
      revisitAt: deferred.revisitAt,
      expectedVersion: recommendation.version,
    });
    expect(JSON.parse(String(request?.body)).clientMutationId).toMatch(
      /^[0-9a-f-]{36}$/,
    );
  });
});
