import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { GmailInflowCandidate } from "../api/gmailInflow";
import { PlanningRequestError } from "../api/planning";
import type { Project } from "../api/projects";
import { copy } from "../copy";
import {
  actionFailureMessage,
  deferDateTimeToIso,
  GmailInflowReview,
  localDateTimeToIso,
  mergeGmailInflowDraftValues,
} from "./GmailInflowReview";

function candidate(
  id: string,
  overrides: Partial<GmailInflowCandidate> = {},
): GmailInflowCandidate {
  return {
    id,
    accountId: `account-${id}`,
    accountEmail: `${id}@company.example`,
    workspaceId: "workspace-company",
    workspaceName: "회사",
    workspaceScope: "company",
    messageId: `message-${id}`,
    providerMessageId: `provider-${id}`,
    providerThreadId: `thread-${id}`,
    senderName: "고객 지원팀",
    senderEmail: "support@example.com",
    subject: `확인 요청 ${id}`,
    snippet: "업무 확인이 필요합니다.",
    bodyText: "첨부한 내용을 확인하고 결과를 알려 주세요.",
    originalThreadUrl: "https://mail.google.com/mail/u/0/#inbox/thread",
    referenceLinks: ["https://docs.example.com/request"],
    receivedAt: "2026-07-30T01:30:00Z",
    suggestedTaskTitle: `요청 ${id} 확인`,
    suggestedTaskNotes: "요청 내용을 확인하고 결과를 공유합니다.",
    suggestedAssigneeName: "조지민",
    suggestedPriority: 2,
    suggestedDueAt: "2026-07-31T08:00:00Z",
    analysisStatus: "ready",
    analysisClassification: "new_task",
    analysisConfidence: 94,
    analysisSummary: "확인이 필요한 업무 요청입니다.",
    analysisErrorCode: null,
    status: "pending",
    promotedTaskId: null,
    deferredUntil: null,
    version: 1,
    ...overrides,
  };
}

function project(id: string, workspaceId = "workspace-company"): Project {
  return {
    id,
    workspaceId,
    title: id,
    status: "active",
  } as Project;
}

describe("Gmail work intake presentation", () => {
  it("keeps personal and company mail visibly separated", () => {
    const markup = renderReview({
      items: [
        candidate("company-mail"),
        candidate("personal-mail", {
          workspaceId: "workspace-personal",
          workspaceName: "개인",
          workspaceScope: "personal",
        }),
      ],
      projects: [
        project("회사 프로젝트"),
        project("개인 프로젝트", "workspace-personal"),
      ],
    });

    expect(markup).toContain("회사");
    expect(markup).toContain("개인");
    expect(markup).toContain("회사 프로젝트");
    expect(markup).not.toContain("개인 프로젝트");
  });

  it("does not silently hide candidates after an arbitrary display limit", () => {
    const markup = renderReview({
      items: Array.from({ length: 21 }, (_, index) =>
        candidate(`mail-${index + 1}`),
      ),
    });

    expect(markup).toContain("확인 요청 mail-21");
    expect(markup).toContain(copy.gmailInflow.count(21));
  });

  it("shows that older mail remains and exposes an explicit load-more action", () => {
    const markup = renderReview({
      hasMore: true,
    });

    expect(markup).toContain(copy.gmailInflow.initialScope);
    expect(markup).toContain(copy.gmailInflow.moreAvailable);
    expect(markup).toContain(copy.gmailInflow.loadMore);
  });

  it("keeps load-more recovery available in an otherwise empty page", () => {
    const markup = renderReview({
      items: [],
      hasMore: true,
      loadingMore: true,
    });

    expect(markup).toContain(copy.gmailInflow.emptyTitle);
    expect(markup).toContain(copy.gmailInflow.loadingMore);
    expect(buttonOpeningTag(markup, copy.gmailInflow.loadingMore)).toContain(
      "disabled",
    );
  });

  it("keeps the next-page retry visible when an empty additional page fails", () => {
    const markup = renderReview({
      items: [],
      hasMore: true,
      loadMoreError: true,
      error: "partial",
    });

    expect(markup).toContain(copy.gmailInflow.moreLoadProblem);
    expect(markup).toContain(copy.gmailInflow.loadMore);
    expect(markup).not.toContain(copy.gmailInflow.loadProblem);
  });

  it("uses a next-page retry instead of resetting loaded results", () => {
    const markup = renderReview({
      hasMore: true,
      loadMoreError: true,
      error: "partial",
    });

    expect(markup).toContain(copy.gmailInflow.moreLoadProblem);
    expect(markup).toContain(copy.gmailInflow.retryLoadMore);
    expect(markup).not.toContain(`>${copy.gmailInflow.reload}</button>`);
  });

  it("keeps an initial workspace warning and full reload action after another workspace loads more", () => {
    const initialError = copy.gmailInflow.initialPartialProblem(["회사"]);
    const markup = renderReview({
      items: [candidate("personal-page")],
      hasMore: true,
      loadMoreError: false,
      error: initialError,
    });

    expect(markup).toContain(initialError);
    expect(markup).toContain(copy.gmailInflow.reload);
    expect(markup).not.toContain(copy.gmailInflow.retryLoadMore);
  });

  it("shows recovery states for loading, full failure, and partial results", () => {
    expect(renderReview({ loading: true, items: [] })).toContain(
      copy.gmailInflow.loading,
    );
    expect(
      renderReview({ error: copy.gmailInflow.loadProblem, items: [] }),
    ).toContain(copy.gmailInflow.loadProblem);
    expect(
      renderReview({
        error: copy.gmailInflow.partialProblem,
        items: [candidate("partial")],
      }),
    ).toContain(copy.gmailInflow.partialProblem);
  });

  it("shows the original source, links, sender, and organized fields", () => {
    const markup = renderReview({
      items: [candidate("source")],
      projects: [project("비스킷링크")],
    });

    expect(markup).toContain("고객 지원팀");
    expect(markup).toContain(copy.gmailInflow.openOriginal);
    expect(markup).toContain("mail.google.com");
    expect(markup).toContain("docs.example.com/request");
    expect(markup).toContain("비스킷링크");
    expect(markup).toContain(copy.gmailInflow.assignee);
    expect(markup).toContain(copy.gmailInflow.priority);
  });

  it("uses the shared zero-based priority contract", () => {
    const markup = renderReview({
      items: [candidate("priority")],
      projects: [project("프로젝트")],
    });

    expect(markup).toContain('option value="0"');
    expect(markup).toContain('option value="1"');
    expect(markup).toContain('option value="2" selected');
    expect(markup).toContain('option value="3"');
    expect(markup).not.toContain('option value="4"');
  });

  it("offers reanalysis and blocks promotion when analysis failed", () => {
    const markup = renderReview({
      items: [
        candidate("failed", {
          analysisStatus: "failed",
          bodyText: null,
        }),
      ],
      projects: [project("프로젝트")],
    });

    expect(markup).toContain(copy.gmailInflow.retryAnalysis);
    expect(markup).toContain(copy.gmailInflow.bodyUnavailable);
    expect(buttonOpeningTag(markup, copy.gmailInflow.promote)).toContain(
      "disabled",
    );
  });

  it("shows a safe diagnostic without exposing the internal analysis code", () => {
    const markup = renderReview({
      items: [
        candidate("failed-code", {
          analysisStatus: "failed",
          analysisErrorCode: "provider.rate_limit_exhausted",
        }),
      ],
      projects: [project("프로젝트")],
    });

    expect(markup).toContain(copy.gmailInflow.analysisDiagnostic);
    expect(markup).not.toContain("provider.rate_limit_exhausted");
  });

  it("keeps non-actionable analysis states out of the home queue", () => {
    const markup = renderReview({
      items: [
        candidate("queued", {
          analysisStatus: "queued",
        }),
      ],
    });

    expect(markup).toContain(copy.gmailInflow.emptyTitle);
    expect(markup).not.toContain("확인 요청 queued");
  });

  it("explains when a deferred candidate returned", () => {
    const deferredUntil = "2026-07-30T01:00:00Z";
    const markup = renderReview({
      items: [
        candidate("deferred", {
          status: "deferred",
          deferredUntil,
        }),
      ],
    });

    expect(markup).toContain(
      copy.gmailInflow.deferredReturned(formatForCopy(deferredUntil)),
    );
  });

  it("routes a reply on a promoted thread back to its linked task", () => {
    const markup = renderReview({
      items: [
        candidate("follow-up", {
          analysisClassification: "follow_up",
          promotedTaskId: "019f0000-0000-7000-8000-000000000001",
        }),
      ],
    });

    expect(markup).toContain(copy.gmailInflow.linkedTaskReplyTitle);
    expect(markup).toContain(copy.gmailInflow.openLinkedTask);
    expect(markup).not.toContain(`>${copy.gmailInflow.promote}</button>`);
  });

  it("shows a calm empty state after every candidate is processed", () => {
    expect(renderReview({ items: [] })).toContain(copy.gmailInflow.emptyTitle);
  });

  it("keeps local deadlines when turning them into API values", () => {
    const value = "2026-07-31T18:30";

    expect(localDateTimeToIso(value)).toBe("2026-07-31T09:30:00.000Z");
    expect(localDateTimeToIso("")).toBeNull();
  });

  it("refreshes untouched suggestions without overwriting edited fields", () => {
    expect(
      mergeGmailInflowDraftValues(
        {
          title: "사용자가 다듬은 제목",
          notes: "이전 정리",
          assigneeName: "김경주",
          priority: 1,
          dueAt: "2026-07-31T18:00",
        },
        {
          title: "새 분석 제목",
          notes: "새 답장을 반영한 정리",
          assigneeName: "주홍석",
          priority: 3,
          dueAt: "2026-08-01T18:00",
        },
        ["title", "assigneeName"],
      ),
    ).toEqual({
      title: "사용자가 다듬은 제목",
      notes: "새 답장을 반영한 정리",
      assigneeName: "김경주",
      priority: 3,
      dueAt: "2026-08-01T18:00",
    });
  });

  it("accepts only a future defer time within one year", () => {
    const now = new Date("2026-07-30T02:00:00Z");
    const future = "2026-07-30T15:00";

    expect(deferDateTimeToIso(future, now)).toBe("2026-07-30T06:00:00.000Z");
    expect(deferDateTimeToIso("2026-07-30T10:00", now)).toBeNull();
    expect(deferDateTimeToIso("2027-08-01T10:00", now)).toBeNull();
  });

  it("uses distinct recovery guidance for a version conflict", () => {
    expect(actionFailureMessage(new PlanningRequestError("conflict"))).toBe(
      copy.gmailInflow.decisionConflict,
    );
    expect(actionFailureMessage(new PlanningRequestError("unavailable"))).toBe(
      copy.gmailInflow.decisionProblem,
    );
  });
});

function renderReview(
  overrides: Partial<ComponentProps<typeof GmailInflowReview>> = {},
): string {
  const props: ComponentProps<typeof GmailInflowReview> = {
    items: [candidate("default")],
    projects: [project("프로젝트")],
    loading: false,
    loadingMore: false,
    loadMoreError: false,
    hasMore: false,
    error: undefined,
    savingId: undefined,
    onReload: () => undefined,
    onLoadMore: () => undefined,
    onPromote: async () => undefined,
    onDismiss: async () => undefined,
    onDefer: async () => undefined,
    onRetryAnalysis: async () => undefined,
    onOpenTask: async () => undefined,
    ...overrides,
  };
  return renderToStaticMarkup(createElement(GmailInflowReview, props));
}

function buttonOpeningTag(markup: string, label: string): string {
  const labelIndex = markup.indexOf(`>${label}</button>`);
  expect(labelIndex).toBeGreaterThanOrEqual(0);
  const buttonIndex = markup.lastIndexOf("<button", labelIndex);
  expect(buttonIndex).toBeGreaterThanOrEqual(0);
  return markup.slice(buttonIndex, markup.indexOf(">", buttonIndex) + 1);
}

function formatForCopy(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}
