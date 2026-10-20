import { afterEach, describe, expect, it, vi } from "vitest";
import { approveInflowChange } from "./inflowChanges";
import {
  retryDeliveryRecovery,
  type DeliveryRecovery,
} from "./deliveryRecovery";
import { fetchReportEvidence } from "./projects";
import type { Task } from "./planning";
import type { ProjectInflowItem } from "./googleChat";

afterEach(() => vi.unstubAllGlobals());
const item = {
  id: "reply",
  version: 2,
  sourceRevision: 3,
} as ProjectInflowItem;
const task = { id: "existing", version: 4 } as Task;
const delivery: DeliveryRecovery = {
  id: "failed",
  kind: "push",
  status: "failed",
  canRetry: true,
  title: "例示",
  projectId: null,
  attemptCount: 3,
  createdAt: "2026-10-08T00:00:00Z",
};
function response(status: number, body?: unknown) {
  const mock = vi.fn<typeof fetch>().mockResolvedValue(
    new Response(body === undefined ? null : JSON.stringify(body), {
      status,
    }),
  );
  vi.stubGlobal("fetch", mock);
  return mock;
}
describe("reply change approval", () => {
  it("sends source, item and current task guards, preserving approved links and explicit deadline removal", async () => {
    const mock = response(204);
    await approveInflowChange(
      "https://example.test/",
      "fixture-access",
      item,
      task,
      "참고 https://example.test/document",
      null,
    );
    expect(mock.mock.calls[0]).toEqual([
      "https://example.test/v1/inflow/reply/change-approval",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "Bearer fixture-access",
        }),
      }),
    ]);
    expect(JSON.parse(String(mock.mock.calls[0][1]?.body))).toEqual({
      expectedItemVersion: 2,
      expectedTaskVersion: 4,
      sourceRevision: 3,
      taskId: "existing",
      notes: "참고 https://example.test/document",
      dueAt: null,
    });
  });
  it.each([
    [401, "unauthorized"],
    [409, "conflict"],
    [503, "unavailable"],
  ])("does not treat status %i as approval", async (status, code) => {
    response(Number(status));
    await expect(
      approveInflowChange(
        "https://example.test",
        "fixture",
        item,
        task,
        "내용",
        null,
      ),
    ).rejects.toMatchObject({ code });
  });
});
describe("separate delivery retry", () => {
  it("retries one receipt without posting source reaction, reply or task content", async () => {
    const mock = response(202);
    await retryDeliveryRecovery("https://example.test/", "fixture", delivery);
    expect(mock.mock.calls[0][0]).toBe(
      "https://example.test/v1/delivery-recovery/push/failed/retry",
    );
    expect(mock.mock.calls[0][1]).toEqual({
      method: "POST",
      headers: { Authorization: "Bearer fixture" },
    });
  });
  it.each([
    [401, "unauthorized"],
    [409, "conflict"],
    [503, "unavailable"],
  ])("keeps failed status %i retryable by the UI", async (status, code) => {
    response(Number(status));
    await expect(
      retryDeliveryRecovery("https://example.test", "fixture", delivery),
    ).rejects.toMatchObject({ code });
  });
});
describe("weekly evidence", () => {
  it("binds project, original report bounds and page cursor in an authenticated GET", async () => {
    const mock = response(200, { items: [task], nextCursor: "cursor" });
    await expect(
      fetchReportEvidence(
        "https://example.test/",
        "fixture",
        "owned",
        "waiting",
        "2026-10-05T00:00:00Z",
        "2026-10-08T00:00:00Z",
        "previous",
      ),
    ).resolves.toEqual({ items: [task], nextCursor: "cursor" });
    const url = new URL(String(mock.mock.calls[0][0]));
    expect(url.pathname).toBe("/v1/projects/owned/weekly-evidence");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      metric: "waiting",
      periodStart: "2026-10-05T00:00:00Z",
      periodEnd: "2026-10-08T00:00:00Z",
      after: "previous",
    });
    expect(mock.mock.calls[0][1]?.headers).toMatchObject({
      Authorization: "Bearer fixture",
    });
  });
  it("rejects malformed evidence instead of replacing rows with invalid data", async () => {
    response(200, { items: null });
    await expect(
      fetchReportEvidence(
        "https://example.test",
        "fixture",
        "owned",
        "waiting",
        "a",
        "b",
      ),
    ).rejects.toMatchObject({ code: "unavailable" });
  });
  it.each([
    [401, "unauthorized"],
    [400, "invalid"],
    [503, "unavailable"],
  ])("exposes status %i without a success state", async (status, code) => {
    response(Number(status));
    await expect(
      fetchReportEvidence(
        "https://example.test",
        "fixture",
        "owned",
        "waiting",
        "a",
        "b",
      ),
    ).rejects.toMatchObject({ code });
  });
});
