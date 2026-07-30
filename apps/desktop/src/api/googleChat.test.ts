import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createProjectGoogleChatSource,
  decideProjectInflow,
  GoogleChatRequestError,
  type ProjectInflowItem,
  type ProjectGoogleChatSource,
  syncProjectGoogleChatSource,
} from "./googleChat";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Google Chat work intake API", () => {
  it("keeps previous messages out unless the user chooses to import them", async () => {
    const fetch = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "source",
          projectId: "project",
          accountId: "account",
        }),
        { status: 201, headers: { "Content-Type": "application/json" } },
      ),
    );
    vi.stubGlobal("fetch", fetch);

    await createProjectGoogleChatSource(
      "https://example.test",
      "access",
      "project",
      {
        accountId: "account",
        spaceName: "spaces/company",
        displayName: "회사 요청",
        acknowledgeWithReaction: true,
        importHistory: false,
      },
    );

    const init = fetch.mock.calls[0]?.[1] as RequestInit;
    expect(JSON.parse(String(init.body))).toMatchObject({
      importHistory: false,
      acknowledgeWithReaction: true,
    });
  });

  it("sends the organized description instead of raw sender-labelled messages", async () => {
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "inflow", status: "promoted" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetch);
    const item = {
      id: "inflow",
      projectId: "project",
      version: 3,
    } as ProjectInflowItem;

    await decideProjectInflow("https://example.test", "access", item, {
      decision: "promote",
      title: "QR 결제 통보 연동 개발",
      notes:
        "업무 목적\nQR 결제 통보 연동 개발\n\n완료 기준\n연동 결과를 공유합니다.",
      priority: 1,
      dueAt: "2026-07-27T14:30:00.000Z",
      withoutDeadline: false,
    });

    const init = fetch.mock.calls[0]?.[1] as RequestInit;
    const body = JSON.parse(String(init.body));
    expect(body.notes).toContain("업무 목적");
    expect(body.notes).not.toContain("보낸 사람 정보 없음");
    expect(body.dueAt).toBe("2026-07-27T14:30:00.000Z");
    expect(body.withoutDeadline).toBe(false);
    expect(body.expectedVersion).toBe(3);
  });

  it("sends an explicit choice when a task has no deadline", async () => {
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "inflow", status: "promoted" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetch);
    const item = {
      id: "inflow",
      projectId: "project",
      version: 4,
    } as ProjectInflowItem;

    await decideProjectInflow("https://example.test", "access", item, {
      decision: "promote",
      title: "기한을 정하지 않은 업무",
      notes: "추후 일정을 정합니다.",
      priority: 1,
      dueAt: null,
      withoutDeadline: true,
    });

    const init = fetch.mock.calls[0]?.[1] as RequestInit;
    expect(JSON.parse(String(init.body))).toMatchObject({
      decision: "promote",
      dueAt: null,
      withoutDeadline: true,
      expectedVersion: 4,
    });
  });

  it("can retry Chat completion delivery for an already promoted item", async () => {
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "inflow", status: "promoted" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetch);
    const item = {
      id: "inflow",
      projectId: "project",
      version: 7,
    } as ProjectInflowItem;

    await decideProjectInflow("https://example.test", "access", item, {
      decision: "retry_completion",
    });

    const init = fetch.mock.calls[0]?.[1] as RequestInit;
    expect(JSON.parse(String(init.body))).toEqual({
      decision: "retry_completion",
      expectedVersion: 7,
    });
  });

  it("keeps the server error code when Chat needs to be reconnected", async () => {
    const fetch = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          error: {
            code: "google_chat.authorization_rejected",
            message: "회사 Google 계정 연결을 다시 진행해 주세요.",
            requestId: "request",
            retryable: false,
            details: {},
          },
        }),
        { status: 400, headers: { "Content-Type": "application/json" } },
      ),
    );
    vi.stubGlobal("fetch", fetch);

    const source = {
      id: "source",
      projectId: "project",
    } as ProjectGoogleChatSource;

    await expect(
      syncProjectGoogleChatSource(
        "https://example.test",
        "access",
        source,
      ),
    ).rejects.toMatchObject({
      name: "GoogleChatRequestError",
      code: "invalid",
      serverCode: "google_chat.authorization_rejected",
      retryable: false,
    } satisfies Partial<GoogleChatRequestError>);
  });
});
