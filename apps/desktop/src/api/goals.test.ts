import { afterEach, describe, expect, it, vi } from "vitest";

import { createGoal, type Goal, updateGoal } from "./goals";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("goal client", () => {
  const goal: Goal = {
    id: "019f68cb-9400-7000-8000-000000000031",
    workspaceId: "019f68cb-9400-7000-8000-000000000032",
    projectId: null,
    title: "반복 업무 줄이기",
    desiredOutcome: "매주 반복 업무 시간을 5시간 줄인다.",
    status: "active",
    targetAt: "2026-08-31T14:59:59.000Z",
    createdAt: "2026-07-20T01:00:00.000Z",
    updatedAt: "2026-07-20T01:00:00.000Z",
    version: 1,
  };

  it("creates an owner-scoped goal", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(goal), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      createGoal("https://jimin-os.example/", "access", {
        workspaceId: goal.workspaceId ?? undefined,
        title: goal.title,
        desiredOutcome: goal.desiredOutcome,
        targetAt: goal.targetAt ?? undefined,
      }),
    ).resolves.toEqual(goal);

    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      workspaceId: goal.workspaceId,
      projectId: null,
      title: goal.title,
      desiredOutcome: goal.desiredOutcome,
      targetAt: goal.targetAt,
    });
  });

  it("updates goal state with optimistic version matching", async () => {
    const updated = { ...goal, status: "achieved", version: 2 } as Goal;
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(updated), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      updateGoal("https://jimin-os.example", "access", goal, {
        workspaceId: goal.workspaceId ?? undefined,
        title: goal.title,
        desiredOutcome: goal.desiredOutcome,
        status: "achieved",
        targetAt: goal.targetAt ?? undefined,
      }),
    ).resolves.toEqual(updated);

    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual(
      expect.objectContaining({ status: "achieved", expectedVersion: 1 }),
    );
  });
});
