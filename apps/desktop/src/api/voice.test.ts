import { afterEach, describe, expect, it, vi } from "vitest";

import { processVoiceCommand } from "./voice";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("voice command API", () => {
  it("sends a recognized request to the server-owned command handler", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response(
          '{"kind":"schedule_created","message":"내일 15:00에 치과 일정을 등록했어요.","destination":"calendar","items":[{"itemType":"schedule","id":"019f68cb-9400-7000-8000-000000000001","title":"치과","dueAt":null,"startsAt":"2026-07-16T15:00:00+09:00","endsAt":"2026-07-16T16:00:00+09:00","priority":null}]}',
          { status: 201, headers: { "Content-Type": "application/json" } },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      processVoiceCommand(
        "https://jimin-os.example/",
        "session-access",
        "내일 오후 3시에 치과 일정 등록해줘",
      ),
    ).resolves.toMatchObject({
      kind: "schedule_created",
      destination: "calendar",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/assistant/voice-commands",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "Bearer session-access",
        }),
      }),
    );
    const request = fetchMock.mock.calls[0]?.[1];
    const requestBody = JSON.parse(String(request?.body)) as {
      clientMutationId: string;
      referenceAt: string;
      text: string;
      timeZone: string;
    };
    expect(requestBody).toMatchObject({
      clientMutationId: expect.stringMatching(
        /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
      ),
      text: "내일 오후 3시에 치과 일정 등록해줘",
      timeZone: expect.any(String),
    });
    expect(requestBody.referenceAt).toMatch(/[+-]\d{2}:\d{2}$/);
  });

  it("accepts a task result that sends the user to today", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn<typeof fetch>()
        .mockResolvedValue(
          new Response(
            '{"kind":"task_created","message":"장보기 할 일을 추가했어요.","destination":"home","items":[{"itemType":"task","id":"019f68cb-9400-7000-8000-000000000002","title":"장보기","dueAt":null,"startsAt":null,"endsAt":null,"priority":1}]}',
            { status: 201, headers: { "Content-Type": "application/json" } },
          ),
        ),
    );

    await expect(
      processVoiceCommand(
        "https://jimin-os.example/",
        "session-access",
        "할 일이 장보기 추가해 줘",
      ),
    ).resolves.toMatchObject({
      kind: "task_created",
      destination: "home",
    });
  });

  it("accepts structured task results for a direct answer", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn<typeof fetch>()
        .mockResolvedValue(
          new Response(
            '{"kind":"tasks_listed","message":"오늘 할 일은 1개예요.","destination":"home","items":[{"itemType":"task","id":"019f68cb-9400-7000-8000-000000000003","title":"계약서 검토","dueAt":"2026-07-15T18:00:00+09:00","startsAt":null,"endsAt":null,"priority":2}]}',
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        ),
    );

    await expect(
      processVoiceCommand(
        "https://jimin-os.example/",
        "session-access",
        "오늘 할 일이 뭐야?",
      ),
    ).resolves.toMatchObject({
      kind: "tasks_listed",
      message: "오늘 할 일은 1개예요.",
      items: [{ title: "계약서 검토" }],
    });
  });
});
