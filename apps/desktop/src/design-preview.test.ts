import { afterEach, describe, expect, it, vi } from "vitest";

import { installDesignPreview } from "./design-preview";
import type { ProjectInflowItem } from "./api/googleChat";
import type { Task } from "./api/planning";

function setupPreview() {
  const originalFetch = vi.fn();
  vi.stubGlobal("window", { fetch: originalFetch });
  vi.stubGlobal("location", { origin: "https://preview.example" });
  installDesignPreview();
  return originalFetch;
}

afterEach(() => vi.unstubAllGlobals());

describe("design preview API isolation", () => {
  it("removes a dismissed candidate from review while retaining its history and original text", async () => {
    const originalFetch = setupPreview();
    const before = await previewHome();
    const candidate = before.inflow[0];
    const response = await decideCandidate(candidate, { decision: "dismiss" });

    expect(response.status).toBe(200);
    const after = await previewHome();
    expect(after.inflow).toHaveLength(25);
    expect(after.tasks).toHaveLength(8);
    const history = await window
      .fetch("/server/v1/projects/preview-project/inflow?status=all")
      .then((response) => response.json());
    expect(
      history.items.find((item: ProjectInflowItem) => item.id === candidate.id),
    ).toMatchObject({
      status: "dismissed",
      contentText: candidate.contentText,
      promotedTaskId: null,
    });
    expect(originalFetch).not.toHaveBeenCalled();
  });

  it("registers a candidate only on submission with the chosen task fields", async () => {
    const originalFetch = setupPreview();
    const candidate = (await previewHome()).inflow[0];
    expect((await previewHome()).tasks).toHaveLength(8);
    const response = await decideCandidate(candidate, {
      decision: "promote",
      title: "담당자가 확인한 할 일",
      notes: "사용자가 정한 완료 조건",
      assigneeName: "송천안",
      priority: 3,
      dueAt: "2026-10-08T09:00:00.000Z",
      withoutDeadline: false,
    });

    expect(response.status).toBe(200);
    const promoted = await response.json();
    const after = await previewHome();
    expect(after.inflow).toHaveLength(25);
    expect(after.tasks).toHaveLength(9);
    expect(
      after.tasks.find(
        (task: { id: string }) => task.id === promoted.promotedTaskId,
      ),
    ).toMatchObject({
      title: "담당자가 확인한 할 일",
      notes: "사용자가 정한 완료 조건",
      assigneeName: "송천안",
      priority: 3,
      dueAt: "2026-10-08T09:00:00.000Z",
      status: "open",
      projectId: candidate.projectId,
    });
    const duplicate = await decideCandidate(candidate, {
      decision: "promote",
      title: "중복 등록",
      notes: "",
      priority: 1,
      dueAt: null,
      withoutDeadline: true,
    });
    expect(duplicate.status).toBe(409);
    expect((await previewHome()).tasks).toHaveLength(9);
    expect(originalFetch).not.toHaveBeenCalled();
  });

  it("requires either a valid deadline or the explicit no-deadline choice", async () => {
    setupPreview();
    const candidate = (await previewHome()).inflow[0];
    const fields = {
      decision: "promote",
      title: "새 할 일",
      notes: "",
      priority: 1,
      dueAt: null,
      withoutDeadline: false,
    };
    expect((await decideCandidate(candidate, fields)).status).toBe(400);
    expect((await previewHome()).inflow).toHaveLength(26);
    expect((await previewHome()).tasks).toHaveLength(8);
    expect(
      (await decideCandidate(candidate, { ...fields, withoutDeadline: true }))
        .status,
    ).toBe(200);
    expect((await previewHome()).tasks.at(-1)).toMatchObject({
      title: "새 할 일",
      dueAt: null,
    });
  });

  it("rejects changed analysis and invalid assignees without changing review or tasks", async () => {
    setupPreview();
    const candidate = (await previewHome()).inflow[0];
    const fields = {
      decision: "promote",
      title: "새 할 일",
      notes: "",
      priority: 1,
      dueAt: null,
      withoutDeadline: true,
    };
    expect(
      (
        await decideCandidate(candidate, {
          ...fields,
          assigneeName: "없는 담당자",
        })
      ).status,
    ).toBe(400);
    const changed = await window.fetch(
      `/server/v1/projects/${candidate.projectId}/inflow/${candidate.id}/decision`,
      {
        method: "POST",
        body: JSON.stringify({
          ...fields,
          expectedVersion: candidate.version,
          conversationId: candidate.conversationId,
          representativeItemId: candidate.representativeItemId,
          expectedSourceRevision: 2,
          expectedAnalyzedRevision: 1,
        }),
      },
    );
    expect(changed.status).toBe(409);
    expect((await previewHome()).tasks).toHaveLength(8);
    expect((await previewHome()).inflow).toHaveLength(26);
  });

  it("loads example tasks without reaching a server", async () => {
    const originalFetch = setupPreview();
    const response = await window.fetch("/server/v1/tasks");
    const result = await response.json();
    expect(result.items).toHaveLength(8);
    expect(result.items[0].id).toBe("preview-task-0");
    expect(originalFetch).not.toHaveBeenCalled();
  });

  it("shows synthetic dated reports without importing private conversations", async () => {
    const originalFetch = setupPreview();
    const response = await window.fetch("/server/v1/home");
    const result = await response.json();
    expect(result.inflow).toHaveLength(26);
    expect(
      new Set(
        result.inflow.map((item: { receivedAt: string }) =>
          new Date(item.receivedAt).toDateString(),
        ),
      ).size,
    ).toBe(7);
    expect(
      result.inflow.every(
        (item: { contentText: string; referenceLinks: string[] }) =>
          item.contentText.startsWith("예시 업무 요청") &&
          item.referenceLinks.length === 0,
      ),
    ).toBe(true);
    expect(originalFetch).not.toHaveBeenCalled();
  });

  it("blocks unsupported actions without sending them to a real server", async () => {
    const originalFetch = setupPreview();
    const response = await window.fetch(
      "https://os.jimin.ai.kr/v1/google-calendar/authorization",
      { method: "POST" },
    );
    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ error: "design_preview_only" });
    expect(originalFetch).not.toHaveBeenCalled();
  });

  it("keeps completion changes inside the temporary example data", async () => {
    const originalFetch = setupPreview();
    await window.fetch("/server/v1/tasks/preview-task-0/complete", {
      method: "POST",
    });
    const response = await window.fetch("/server/v1/tasks?status=all");
    const result = await response.json();
    expect(result.items[0].status).toBe("completed");
    expect(originalFetch).not.toHaveBeenCalled();
  });
});

async function previewHome() {
  return window
    .fetch("/server/v1/home")
    .then((response) => response.json()) as Promise<{
    inflow: ProjectInflowItem[];
    tasks: Task[];
  }>;
}

function decideCandidate(
  item: ProjectInflowItem,
  fields: Record<string, unknown>,
) {
  return window.fetch(
    `/server/v1/projects/${item.projectId}/inflow/${item.id}/decision`,
    {
      method: "POST",
      body: JSON.stringify({
        ...fields,
        expectedVersion: item.version,
        conversationId: item.conversationId,
        representativeItemId: item.representativeItemId,
        expectedSourceRevision: item.sourceRevision,
        expectedAnalyzedRevision: item.analyzedRevision,
      }),
    },
  );
}
