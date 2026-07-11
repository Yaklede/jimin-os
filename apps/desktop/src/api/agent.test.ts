import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchAgentJob, queueAgentTurn } from "./agent";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("agent API", () => {
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
    vi.stubGlobal("crypto", { randomUUID: () => "message-client-1" });

    const queued = await queueAgentTurn(
      "https://jimin-os.example/",
      "session-access",
      "conversation-1",
      "오늘 할 일을 정리해줘",
    );

    expect(queued.state).toBe("queued");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/conversations/conversation-1/turns",
      expect.objectContaining({ method: "POST" }),
    );
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
});
