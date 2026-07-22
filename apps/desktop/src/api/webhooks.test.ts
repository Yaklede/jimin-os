import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createProjectWebhook,
  deleteProjectWebhook,
  fetchProjectWebhooks,
  parseWebhookMentionDirectory,
  retryWebhookDelivery,
  testProjectWebhook,
  type ProjectWebhook,
  updateProjectWebhook,
} from "./webhooks";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("project webhook client", () => {
  const webhook: ProjectWebhook = {
    id: "019f68cb-9400-7000-8000-000000000031",
    projectId: "019f68cb-9400-7000-8000-000000000032",
    provider: "discord",
    destinationLabel: "Discord 채널",
    mentionDirectory: { users: {} },
    events: ["task.created", "task.completed"],
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
          url: "https://discord.com/api/webhooks/123/private",
          provider: "discord",
          events: webhook.events,
          mentionDirectory: { users: {} },
        },
      ),
    ).resolves.toEqual(webhook);

    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toEqual({
      provider: "discord",
      url: "https://discord.com/api/webhooks/123/private",
      events: webhook.events,
      mentionDirectory: { users: {} },
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

  it("updates a webhook without echoing a stored secret", async () => {
    const updated = { ...webhook, enabled: false, version: 4 };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(updated), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      updateProjectWebhook("https://jimin-os.example", "access", webhook, {
        provider: "discord",
        destinationMode: "keep",
        events: webhook.events,
        enabled: false,
        mentionDirectory: { users: {} },
      }),
    ).resolves.toEqual(updated);

    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: "PUT" });
    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
      provider: "discord",
      destinationMode: "keep",
      url: null,
      events: webhook.events,
      enabled: false,
      mentionDirectory: { users: {} },
      expectedVersion: 3,
    });
  });

  it("parses the editable Google Chat user directory", () => {
    expect(
      parseWebhookMentionDirectory(`{
        "users": {
          "홍길동": "users/123456789012345678901",
          "김개발": "users/987654321098765432109"
        }
      }`),
    ).toEqual({
      users: {
        홍길동: "users/123456789012345678901",
        김개발: "users/987654321098765432109",
      },
    });
    expect(
      parseWebhookMentionDirectory(
        '{"users":{"홍길동":"123456789012345678901"}}',
      ),
    ).toBeUndefined();
    expect(
      parseWebhookMentionDirectory('{"users":{},"unexpected":true}'),
    ).toBeUndefined();
    expect(
      parseWebhookMentionDirectory(
        '{"users":{"홍길동":"users/123456789012345678901"},,}',
      ),
    ).toBeUndefined();
  });

  it("keeps the webhook screen usable during a server rollout", async () => {
    const legacyWebhook = { ...webhook } as Partial<ProjectWebhook>;
    delete legacyWebhook.mentionDirectory;
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(
          JSON.stringify({ items: [legacyWebhook], nextCursor: null }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      ),
    );

    await expect(
      fetchProjectWebhooks(
        "https://jimin-os.example",
        "access",
        webhook.projectId,
      ),
    ).resolves.toEqual([{ ...webhook, mentionDirectory: { users: {} } }]);
  });

  it("requeues a failed delivery with its stable delivery identifier", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(null, { status: 202 }));
    vi.stubGlobal("fetch", fetchMock);

    await retryWebhookDelivery(
      "https://jimin-os.example",
      "access",
      webhook.projectId,
      "019f68cb-9400-7000-8000-000000000033",
    );

    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/projects/019f68cb-9400-7000-8000-000000000032/webhook-deliveries/019f68cb-9400-7000-8000-000000000033/retry",
      expect.objectContaining({ method: "POST" }),
    );
  });
});
