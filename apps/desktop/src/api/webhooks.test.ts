import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createProjectWebhook,
  deleteProjectWebhook,
  testProjectWebhook,
  type ProjectWebhook,
} from "./webhooks";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("project webhook client", () => {
  const webhook: ProjectWebhook = {
    id: "019f68cb-9400-7000-8000-000000000031",
    projectId: "019f68cb-9400-7000-8000-000000000032",
    url: "https://automation.example/hooks/jimin",
    events: ["task.created", "task.completed"],
    hasAuthentication: true,
    enabled: true,
    version: 3,
  };

  it("creates a webhook without retaining its authorization value", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(webhook), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      createProjectWebhook(
        "https://jimin-os.example/",
        "access",
        webhook.projectId,
        {
          url: webhook.url,
          events: webhook.events,
          authorization: "Bearer private",
        },
      ),
    ).resolves.toEqual(webhook);

    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      url: webhook.url,
      events: webhook.events,
      authorization: "Bearer private",
    });
  });

  it("queues a test delivery and deletes with optimistic version matching", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(new Response(null, { status: 202 }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await testProjectWebhook("https://jimin-os.example", "access", webhook);
    await deleteProjectWebhook("https://jimin-os.example", "access", webhook);

    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: "POST" });
    expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: "DELETE" });
    expect(JSON.parse(String(fetchMock.mock.calls[1]?.[1]?.body))).toEqual({
      expectedVersion: 3,
    });
  });
});
