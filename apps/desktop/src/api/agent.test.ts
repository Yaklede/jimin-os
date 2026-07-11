import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createConversation,
  fetchAgentJob,
  fetchLatestConversationJob,
  queueAgentTurn,
} from "./agent";
import { createUuidV7 } from "../uuid";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("agent API", () => {
  it("creates version-seven IDs for retry-safe client turns", () => {
    vi.stubGlobal("crypto", {
      getRandomValues: (value: Uint8Array) => value.fill(0),
    });
    vi.spyOn(Date, "now").mockReturnValue(1_784_169_600_000);

    expect(createUuidV7()).toMatch(/^019f68cb-9400-7000-8000-000000000000$/);
  });

  it("queues a text turn against the selected conversation", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"jobId":"job-1","messageId":"message-1","conversationId":"conversation-1","state":"queued"}',
          { status: 202, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);

    const queued = await queueAgentTurn(
      "https://jimin-os.example/",
      "session-access",
      "conversation-1",
      "오늘 할 일을 정리해줘",
      "message-client-1",
    );

    expect(queued.state).toBe("queued");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/conversations/conversation-1/turns",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("creates a conversation with a client-held retry identifier", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"id":"conversation-1","title":"오늘 일정","status":"active","lastMessageAt":null,"version":1}',
          { status: 201, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);

    await createConversation(
      "https://jimin-os.example/",
      "session-access",
      "019f68cb-9400-7000-8000-000000000000",
      "오늘 일정",
    );

    const request = fetchMock.mock.calls[0]?.[1];
    expect(JSON.parse(String(request?.body))).toMatchObject({
      clientConversationId: "019f68cb-9400-7000-8000-000000000000",
      title: "오늘 일정",
    });
  });

  it("returns the current job state without exposing server internals", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn<typeof fetch>()
        .mockResolvedValue(
          new Response(
            '{"id":"job-1","conversationId":"conversation-1","state":"running","createdAt":"2026-07-11T00:00:00Z","finishedAt":null,"version":2}',
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        ),
    );

    const job = await fetchAgentJob(
      "https://jimin-os.example",
      "session-access",
      "job-1",
    );

    expect(job.state).toBe("running");
  });

  it("restores the newest job for a conversation after reopening it", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"id":"job-2","conversationId":"conversation-1","state":"failed","createdAt":"2026-07-11T00:00:00Z","finishedAt":"2026-07-11T00:01:00Z","version":3}',
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);

    const job = await fetchLatestConversationJob(
      "https://jimin-os.example/",
      "session-access",
      "conversation-1",
    );

    expect(job?.state).toBe("failed");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/conversations/conversation-1/jobs/latest",
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: "Bearer session-access",
        }),
      }),
    );
  });

  it("keeps a conversation without requests free of a status message", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn<typeof fetch>()
        .mockResolvedValue(new Response(null, { status: 204 })),
    );

    await expect(
      fetchLatestConversationJob(
        "https://jimin-os.example",
        "session-access",
        "conversation-1",
      ),
    ).resolves.toBeUndefined();
  });
});
