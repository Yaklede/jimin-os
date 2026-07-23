import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createProjectGoogleChatSource,
  decideProjectInflow,
  type ProjectInflowItem,
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
    });

    const init = fetch.mock.calls[0]?.[1] as RequestInit;
    const body = JSON.parse(String(init.body));
    expect(body.notes).toContain("업무 목적");
    expect(body.notes).not.toContain("보낸 사람 정보 없음");
    expect(body.expectedVersion).toBe(3);
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
});
