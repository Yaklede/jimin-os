import { afterEach, describe, expect, it, vi } from "vitest";

import { type Project, updateProject } from "./projects";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("project client", () => {
  it("replaces mutable project fields with optimistic version matching", async () => {
    const updated: Project = {
      id: "019f68cb-9400-7000-8000-000000000001",
      workspaceId: "019f68cb-9400-7000-8000-000000000002",
      title: "개인 AI 비서",
      objective: "업무 판단과 실행 연결",
      status: "paused",
      riskLevel: 2,
      nextAction: "Webhook 계약 확정",
      dueAt: "2026-07-20T14:59:59.000Z",
      openTaskCount: 2,
      version: 4,
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(updated), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const current = { ...updated, status: "active", version: 3 } as Project;
    await expect(
      updateProject("https://jimin-os.example/", "access", current, {
        title: updated.title,
        objective: updated.objective ?? undefined,
        status: updated.status,
        riskLevel: updated.riskLevel,
        nextAction: updated.nextAction ?? undefined,
        dueAt: updated.dueAt ?? undefined,
      }),
    ).resolves.toEqual(updated);

    expect(fetchMock).toHaveBeenCalledWith(
      `https://jimin-os.example/v1/projects/${updated.id}`,
      expect.objectContaining({ method: "PUT" }),
    );
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      title: updated.title,
      objective: updated.objective,
      status: updated.status,
      riskLevel: updated.riskLevel,
      nextAction: updated.nextAction,
      dueAt: updated.dueAt,
      expectedVersion: 3,
    });
  });
});
