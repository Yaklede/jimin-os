import { afterEach, describe, expect, it, vi } from "vitest";

import { deleteProject, type Project, updateProject } from "./projects";

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
      managementMode: "operation",
      reportingEnabled: true,
      staleThresholdDays: 7,
      riskLevel: 2,
      nextAction: "Webhook 계약 확정",
      dueAt: "2026-07-20T14:59:59.000Z",
      openTaskCount: 2,
      totalTaskCount: 4,
      completedTaskCount: 2,
      overdueTaskCount: 1,
      unassignedTaskCount: 0,
      progressPercent: 50,
      weeklyCreatedTaskCount: 5,
      weeklyCompletedTaskCount: 3,
      backlogDelta: 2,
      staleTaskCount: 1,
      averageCycleTimeHours: 18,
      onTimeCompletionPercent: 75,
      health: "at_risk",
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
        managementMode: updated.managementMode,
        reportingEnabled: updated.reportingEnabled,
        staleThresholdDays: updated.staleThresholdDays,
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
      managementMode: updated.managementMode,
      reportingEnabled: updated.reportingEnabled,
      staleThresholdDays: updated.staleThresholdDays,
      riskLevel: updated.riskLevel,
      nextAction: updated.nextAction,
      dueAt: updated.dueAt,
      expectedVersion: 3,
    });
  });

  it("deletes a project with optimistic version matching", async () => {
    const project: Project = {
      id: "019f68cb-9400-7000-8000-000000000001",
      workspaceId: "019f68cb-9400-7000-8000-000000000002",
      title: "개인 AI 비서",
      objective: null,
      status: "active",
      managementMode: "completion",
      reportingEnabled: true,
      staleThresholdDays: 7,
      riskLevel: 0,
      nextAction: null,
      dueAt: null,
      openTaskCount: 2,
      totalTaskCount: 2,
      completedTaskCount: 0,
      overdueTaskCount: 0,
      unassignedTaskCount: 2,
      progressPercent: 0,
      weeklyCreatedTaskCount: 0,
      weeklyCompletedTaskCount: 0,
      backlogDelta: 0,
      staleTaskCount: 0,
      averageCycleTimeHours: 0,
      onTimeCompletionPercent: null,
      health: "needs_plan",
      version: 4,
    };
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      deleteProject("https://jimin-os.example/", "access", project),
    ).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith(
      `https://jimin-os.example/v1/projects/${project.id}`,
      expect.objectContaining({ method: "DELETE" }),
    );
    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({ expectedVersion: 4 });
  });
});
