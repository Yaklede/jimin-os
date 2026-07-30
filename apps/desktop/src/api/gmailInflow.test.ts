import { afterEach, describe, expect, it, vi } from "vitest";

import {
  decideGmailInflow,
  emptyGmailInflowLoadHealth,
  fetchGmailInflow,
  gmailInflowHealthAfterInitial,
  gmailInflowHealthAfterLoadMore,
  isTrustedGmailUrl,
  normalizeGmailInflowCandidate,
  type GmailInflowCandidate,
} from "./gmailInflow";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const candidate: GmailInflowCandidate = {
  id: "019f6e57-2c00-7000-8000-000000000001",
  accountId: "019f6e57-2c00-7000-8000-000000000002",
  accountEmail: "owner@company.example",
  workspaceId: "019f6e57-2c00-7000-8000-000000000003",
  workspaceName: "회사",
  workspaceScope: "company",
  messageId: "019f6e57-2c00-7000-8000-000000000004",
  providerMessageId: "provider-message",
  providerThreadId: "provider-thread",
  senderName: "고객 지원팀",
  senderEmail: "support@example.com",
  subject: "계약서 검토 요청",
  snippet: "금요일까지 계약서를 확인해 주세요.",
  bodyText: "첨부한 계약서를 검토하고 의견을 회신해 주세요.",
  originalThreadUrl: "https://mail.google.com/mail/u/0/#inbox/thread",
  referenceLinks: ["https://docs.example.com/contract"],
  receivedAt: "2026-07-30T01:30:00Z",
  suggestedTaskTitle: "계약서 검토 의견 회신",
  suggestedTaskNotes: "첨부 계약서를 검토하고 의견을 회신합니다.",
  suggestedAssigneeName: "조지민",
  suggestedPriority: 2,
  suggestedDueAt: "2026-07-31T08:00:00Z",
  analysisStatus: "ready",
  analysisClassification: "new_task",
  analysisConfidence: 94,
  analysisSummary: "계약서 검토 후 회신이 필요한 요청입니다.",
  analysisErrorCode: null,
  status: "pending",
  promotedTaskId: null,
  deferredUntil: null,
  version: 3,
};

describe("Gmail work intake API", () => {
  it("preserves an initial workspace failure across load-more and clears it only after a successful reload", () => {
    const afterCompanyInitialFailure = gmailInflowHealthAfterInitial(["회사"]);
    const afterPersonalLoadMore = gmailInflowHealthAfterLoadMore(
      afterCompanyInitialFailure,
      [],
    );

    expect(afterPersonalLoadMore).toEqual({
      initialFailedWorkspaces: ["회사"],
      loadMoreFailedWorkspaces: [],
    });
    expect(
      gmailInflowHealthAfterLoadMore(afterCompanyInitialFailure, ["개인"]),
    ).toEqual({
      initialFailedWorkspaces: ["회사"],
      loadMoreFailedWorkspaces: ["개인"],
    });
    expect(gmailInflowHealthAfterInitial([])).toEqual(
      emptyGmailInflowLoadHealth,
    );
  });

  it("loads attention items for only the selected workspace", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ items: [candidate], nextCursor: null }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailInflow(
        "https://jimin-os.example/",
        "access",
        candidate.workspaceId,
      ),
    ).resolves.toEqual({
      items: [candidate],
      nextCursor: null,
      partial: false,
    });
    const requested = new URL(String(fetchMock.mock.calls[0]?.[0]));
    expect(requested.pathname).toBe("/v1/gmail/inflow");
    expect(requested.searchParams.get("workspaceId")).toBe(
      candidate.workspaceId,
    );
    expect(requested.searchParams.get("status")).toBe("attention");
    expect(requested.searchParams.get("limit")).toBe("100");
    expect(requested.searchParams.has("cursor")).toBe(false);
  });

  it("loads only the first 100 automatically and returns the next cursor", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({ items: [candidate], nextCursor: "page-2" }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailInflow(
        "https://jimin-os.example",
        "access",
        candidate.workspaceId,
      ),
    ).resolves.toEqual({
      items: [candidate],
      nextCursor: "page-2",
      partial: false,
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("loads the next page only when its opaque cursor is supplied", async () => {
    const second = {
      ...candidate,
      id: "019f6e57-2c00-7000-8000-000000000099",
      providerMessageId: "provider-message-2",
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ items: [second], nextCursor: null }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailInflow(
        "https://jimin-os.example",
        "access",
        candidate.workspaceId,
        "opaque-page-2",
      ),
    ).resolves.toEqual({
      items: [second],
      nextCursor: null,
      partial: false,
    });
    expect(
      new URL(String(fetchMock.mock.calls[0]?.[0])).searchParams.get("cursor"),
    ).toBe("opaque-page-2");
    expect(
      new URL(String(fetchMock.mock.calls[0]?.[0])).searchParams.get("limit"),
    ).toBe("100");
  });

  it("promotes into a project in the same workspace with the suggested fields", async () => {
    const promoted = {
      ...candidate,
      status: "promoted",
      promotedTaskId: "019f6e57-2c00-7000-8000-000000000005",
      version: 4,
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(promoted), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await decideGmailInflow("https://jimin-os.example", "access", candidate, {
      decision: "promote",
      projectId: "project-company",
      title: candidate.suggestedTaskTitle,
      notes: candidate.suggestedTaskNotes,
      assigneeName: "조지민",
      priority: 2,
      dueAt: candidate.suggestedDueAt,
      withoutDeadline: false,
    });

    expect(
      JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body)),
    ).toMatchObject({
      decision: "promote",
      projectId: "project-company",
      assigneeName: "조지민",
      priority: 2,
      dueAt: candidate.suggestedDueAt,
      withoutDeadline: false,
      expectedVersion: 3,
    });
  });

  it("sends an explicit revisit time when deferring", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-30T02:00:00Z"));
    const revisitAt = "2026-07-30T06:00:00Z";
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({ ...candidate, status: "deferred", version: 4 }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await decideGmailInflow("https://jimin-os.example", "access", candidate, {
      decision: "defer",
      revisitAt,
    });

    expect(
      JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body)),
    ).toMatchObject({
      decision: "defer",
      revisitAt,
      expectedVersion: 3,
    });
  });

  it("rejects an out-of-range task priority before requesting", async () => {
    const fetchMock = vi.fn<typeof fetch>();
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      decideGmailInflow("https://jimin-os.example", "access", candidate, {
        decision: "promote",
        projectId: "project-company",
        title: "검토",
        notes: "",
        assigneeName: null,
        priority: 4,
        dueAt: null,
        withoutDeadline: true,
      }),
    ).rejects.toMatchObject({ code: "invalid" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("keeps only trusted Gmail original links", () => {
    expect(isTrustedGmailUrl(candidate.originalThreadUrl)).toBe(true);
    expect(isTrustedGmailUrl("https://example.com/fake-gmail")).toBe(false);
    const normalized = normalizeGmailInflowCandidate({
      ...candidate,
      originalThreadUrl: "https://example.com/fake-gmail",
      referenceLinks: [
        "javascript:alert(1)",
        "https://docs.example.com/contract",
      ],
    });
    expect(normalized.originalThreadUrl).toBeNull();
    expect(normalized.referenceLinks).toEqual([
      "https://docs.example.com/contract",
    ]);
  });

  it("accepts a claimed message while a worker is analyzing it", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          items: [{ ...candidate, analysisStatus: "claimed" }],
          nextCursor: null,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailInflow(
        "https://jimin-os.example",
        "access",
        candidate.workspaceId,
      ),
    ).resolves.toMatchObject({
      items: [{ analysisStatus: "claimed" }],
      partial: false,
    });
  });

  it("normalizes nullable Gmail metadata returned by the API", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          items: [
            {
              ...candidate,
              senderEmail: null,
              subject: null,
              snippet: null,
              receivedAt: null,
            },
          ],
          nextCursor: null,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailInflow(
        "https://jimin-os.example",
        "access",
        candidate.workspaceId,
      ),
    ).resolves.toMatchObject({
      items: [
        {
          senderEmail: "",
          subject: "",
          snippet: "",
          receivedAt: "",
        },
      ],
      partial: false,
    });
  });

  it("rejects a missing cursor contract instead of silently ending early", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ items: [candidate] }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailInflow(
        "https://jimin-os.example",
        "access",
        candidate.workspaceId,
      ),
    ).rejects.toMatchObject({ code: "unavailable" });
  });

  it("rejects out-of-range suggestions and malformed timestamps", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          items: [
            {
              ...candidate,
              suggestedPriority: 4,
              receivedAt: "not-a-time",
            },
          ],
          nextCursor: null,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailInflow(
        "https://jimin-os.example",
        "access",
        candidate.workspaceId,
      ),
    ).rejects.toMatchObject({ code: "unavailable" });
  });
});
