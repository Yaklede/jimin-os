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
          '{"kind":"schedule_created","message":"내일 15:00에 치과 일정을 등록했어요.","destination":"calendar"}',
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
    expect(JSON.parse(String(request?.body))).toMatchObject({
      text: "내일 오후 3시에 치과 일정 등록해줘",
      timeZone: expect.any(String),
    });
  });

  it("accepts a task result that sends the user to today", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn<typeof fetch>()
        .mockResolvedValue(
          new Response(
            '{"kind":"task_created","message":"장보기 할 일을 추가했어요.","destination":"home"}',
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
});
