import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createProjectGoogleChatSource,
  decideProjectInflow,
  GoogleChatRequestError,
  normalizeProjectInflowItem,
  projectInflowPromotionReadiness,
  type ProjectInflowItem,
  type ProjectGoogleChatSource,
  syncProjectGoogleChatSource,
} from "./googleChat";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Google Chat work intake API", () => {
  it("keeps missing analysis identity explicit instead of guessing from the item", () => {
    const normalized = normalizeProjectInflowItem({
      ...promotableItem(7),
      conversationId: null,
      representativeItemId: null,
      sourceRevision: null,
      analyzedRevision: null,
      referenceDocuments: undefined,
    } as unknown as ProjectInflowItem);

    expect(normalized.conversationId).toBeNull();
    expect(normalized.representativeItemId).toBeNull();
    expect(normalized.sourceRevision).toBeNull();
    expect(normalized.analyzedRevision).toBeNull();
    expect(normalized.referenceDocuments).toEqual([]);
  });

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
    const item = promotableItem(3);

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
    expect(body).toEqual({
      decision: "promote",
      title: "QR 결제 통보 연동 개발",
      notes:
        "업무 목적\nQR 결제 통보 연동 개발\n\n완료 기준\n연동 결과를 공유합니다.",
      priority: 1,
      dueAt: "2026-07-27T14:30:00.000Z",
      withoutDeadline: false,
      conversationId: "019f0000-0000-7000-8000-000000000101",
      representativeItemId: "019f0000-0000-7000-8000-000000000102",
      expectedSourceRevision: 4,
      expectedAnalyzedRevision: 4,
      expectedVersion: 3,
    });
    expect(JSON.stringify(body)).not.toContain("referenceDocuments");
    expect(JSON.stringify(body)).not.toContain("referenceLinks");
    expect(JSON.stringify(body)).not.toContain("원문 전체 내용");
  });

  it("sends an explicit choice when a task has no deadline", async () => {
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "inflow", status: "promoted" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetch);
    const item = promotableItem(4);

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
      conversationId: "019f0000-0000-7000-8000-000000000101",
      representativeItemId: "019f0000-0000-7000-8000-000000000102",
      expectedSourceRevision: 4,
      expectedAnalyzedRevision: 4,
    });
  });

  it.each([
    [
      "analysis is refreshing",
      { analysisStatus: "refreshing" as const },
      "analysis_not_ready",
    ],
    [
      "analysis is not a new task",
      { analysisClassification: "status_update" as const },
      "not_actionable",
    ],
    [
      "conversation identity is missing",
      { conversationId: null },
      "missing_context",
    ],
    [
      "representative identity is missing",
      { representativeItemId: null },
      "missing_context",
    ],
    ["source revision is missing", { sourceRevision: null }, "missing_context"],
    [
      "analysis revision is invalid",
      { analyzedRevision: 0 },
      "missing_context",
    ],
    ["item version is invalid", { version: 0 }, "missing_context"],
    ["analysis is stale", { analyzedRevision: 3 }, "analysis_stale"],
  ])("blocks promotion before fetch when %s", async (_, overrides, reason) => {
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    const item = { ...promotableItem(5), ...overrides };

    expect(projectInflowPromotionReadiness(item)).toEqual({
      canPromote: false,
      reason,
    });
    await expect(
      decideProjectInflow("https://example.test", "access", item, {
        decision: "promote",
        title: "요청 확인",
        notes: "요청을 확인합니다.",
        priority: 1,
        dueAt: null,
        withoutDeadline: true,
      }),
    ).rejects.toMatchObject({
      code: "conflict",
      serverCode: "project.inflow_analysis_changed",
    });
    expect(fetch).not.toHaveBeenCalled();
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
      syncProjectGoogleChatSource("https://example.test", "access", source),
    ).rejects.toMatchObject({
      name: "GoogleChatRequestError",
      code: "invalid",
      serverCode: "google_chat.authorization_rejected",
      retryable: false,
    } satisfies Partial<GoogleChatRequestError>);
  });
});

function promotableItem(version: number): ProjectInflowItem {
  return {
    id: "019f0000-0000-7000-8000-000000000100",
    conversationId: "019f0000-0000-7000-8000-000000000101",
    representativeItemId: "019f0000-0000-7000-8000-000000000102",
    projectId: "019f0000-0000-7000-8000-000000000103",
    analysisStatus: "ready",
    analysisClassification: "new_task",
    sourceRevision: 4,
    analyzedRevision: 4,
    referenceLinks: ["https://itsm.example/issues/3876"],
    referenceDocuments: [
      {
        provider: "itsm",
        url: "https://itsm.example/issues/3876",
        externalId: "3876",
        title: "정산 방식 표기 요청",
        originalContent: "원문 전체 내용",
        errorCode: null,
      },
    ],
    version,
  } as ProjectInflowItem;
}
