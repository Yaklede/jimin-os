import { afterEach, expect, it, vi } from "vitest";
import { completionReviewRequest } from "./completionReviews";
afterEach(() => vi.unstubAllGlobals());
it("sends owner auth and both concurrency fences without sending a task completion itself", async () => {
  const fetcher = vi
    .fn()
    .mockResolvedValue(new Response('{"status":"approved"}', { status: 200 }));
  vi.stubGlobal("fetch", fetcher);
  const body = {
    kind: "approve",
    expectedVersion: 1,
    expectedTaskVersion: 7,
    note: null,
  };
  await expect(
    completionReviewRequest(
      "http://localhost:9999/",
      "owner-token",
      "/review/decision",
      body,
    ),
  ).resolves.toEqual({ status: "approved" });
  expect(fetcher).toHaveBeenCalledWith(
    "http://localhost:9999/v1/task-completion-reviews/review/decision",
    expect.objectContaining({
      method: "POST",
      headers: expect.objectContaining({ Authorization: "Bearer owner-token" }),
      body: JSON.stringify(body),
    }),
  );
});
it.each([
  [401, "unauthorized"],
  [409, "conflict"],
  [400, "invalid"],
  [503, "unavailable"],
])("classifies %s safely", async (status, code) => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue(new Response("", { status: status as number })),
  );
  await expect(
    completionReviewRequest("http://localhost:9999", "token"),
  ).rejects.toMatchObject({ code });
});
