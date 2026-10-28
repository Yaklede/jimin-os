import { afterEach, describe, expect, it, vi } from "vitest";
import {
  chatRoutingRequest,
  deleteTaskChatRoute,
  saveTaskChatRoute,
  updateChatSourcePurpose,
} from "./chatRouting";
import type { ProjectGoogleChatSource } from "./googleChat";

afterEach(() => vi.unstubAllGlobals());
describe("project Chat routing API", () => {
  it("keeps explicit source, mention directory and version in one owner-scoped save", async () => {
    const fetcher = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          item: {
            projectId: "p",
            sourceId: "dev",
            directoryWebhookId: "people",
            version: 4,
          },
        }),
      ),
    );
    vi.stubGlobal("fetch", fetcher);
    await saveTaskChatRoute("https://example.test/", "fixture-access", "p", {
      sourceId: "dev",
      directoryWebhookId: "people",
      expectedVersion: 3,
    });
    const [url, init] = fetcher.mock.calls[0];
    expect(url).toBe("https://example.test/v1/projects/p/chat-task-routing");
    expect(init.method).toBe("PUT");
    expect(JSON.parse(init.body)).toEqual({
      sourceId: "dev",
      directoryWebhookId: "people",
      expectedVersion: 3,
    });
    expect(init.headers.Authorization).toBe("Bearer fixture-access");
  });
  it("uses the current source version when changing collection/reporting purpose", async () => {
    const fetcher = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ id: "s", purpose: "delivery" })),
      );
    vi.stubGlobal("fetch", fetcher);
    await updateChatSourcePurpose(
      "https://example.test",
      "fixture-access",
      { id: "s", projectId: "p", version: 7 } as ProjectGoogleChatSource,
      "delivery",
    );
    expect(fetcher.mock.calls[0][0]).toBe(
      "https://example.test/v1/projects/p/google-chat-sources/s/purpose",
    );
    expect(JSON.parse(fetcher.mock.calls[0][1].body)).toEqual({
      purpose: "delivery",
      expectedVersion: 7,
    });
  });
  it("does not parse an empty successful unlink response", async () => {
    const fetcher = vi
      .fn()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetcher);
    await expect(
      deleteTaskChatRoute("https://example.test", "fixture-access", {
        projectId: "p",
        sourceId: "s",
        directoryWebhookId: "h",
        version: 9,
      }),
    ).resolves.toBeUndefined();
    expect(fetcher.mock.calls[0][0]).toBe(
      "https://example.test/v1/projects/p/chat-task-routing?expected_version=9",
    );
  });
  it.each([
    [401, "unauthorized"],
    [409, "conflict"],
    [400, "invalid"],
    [422, "invalid"],
    [503, "unavailable"],
  ])(
    "preserves %s for session refresh or explicit conflict recovery",
    async (status, code) => {
      vi.stubGlobal(
        "fetch",
        vi
          .fn()
          .mockResolvedValue(new Response("{}", { status: Number(status) })),
      );
      await expect(
        chatRoutingRequest(
          "https://example.test",
          "fixture-access",
          "p/chat-task-routing",
        ),
      ).rejects.toMatchObject({ code });
    },
  );
});
