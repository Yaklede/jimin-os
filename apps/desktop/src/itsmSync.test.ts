import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { itsmSyncCandidates, itsmSyncHelp } from "./itsmSync";
import type { ProjectInflowReferenceDocument } from "./api/googleChat";
import { DeliveryRecoveryPanel } from "./components/DeliveryRecoveryPanel";
describe("ITSM task synchronization", () => {
  const document: ProjectInflowReferenceDocument = {
    provider: "itsm",
    externalId: "42",
    url: "https://itsm.example/issues/42?tab=history",
    title: "결제 오류",
    originalContent: null,
    errorCode: null,
  };
  it("offers only verified ticket evidence and canonicalizes duplicate links", () => {
    expect(
      itsmSyncCandidates([
        document,
        document,
        { ...document, url: "https://itsm.example/issues/99" },
        { ...document, errorCode: "itsm.unavailable" },
        { ...document, url: "http://itsm.example/issues/42" },
      ]),
    ).toEqual(["https://itsm.example/issues/42"]);
  });
  it("separates ITSM retries from Chat and push and exposes an actionable error", () => {
    const html = renderToStaticMarkup(
      createElement(DeliveryRecoveryPanel, {
        items: [
          {
            id: "42",
            kind: "itsm",
            title: "결제 오류",
            status: "failed",
            canRetry: true,
            attemptCount: 1,
            projectId: null,
            createdAt: "2026-10-20T00:00:00Z",
            errorCode: "itsm.workflow_blocked",
            referenceUrl: document.url,
          },
        ],
        onRetry: async () => {},
      }),
    );
    expect(html).toContain("ITSM에 다시 반영하기");
    expect(html).toContain("연결된 ITSM 티켓 보기");
    expect(html).toContain("상태 전환 규칙");
    expect(html).not.toContain("이 알림만 다시 보내기");
    expect(itsmSyncHelp("itsm.verification_failed")).toContain("실제 값");
  });
  it("shows queued ITSM reflection as pending without a duplicate retry action", () => {
    const html = renderToStaticMarkup(
      createElement(DeliveryRecoveryPanel, {
        items: [
          {
            id: "a",
            kind: "itsm",
            title: "예시",
            status: "queued",
            canRetry: false,
            attemptCount: 0,
            projectId: null,
            createdAt: "2026-10-20T00:00:00Z",
          },
        ],
        onRetry: async () => {},
      }),
    );
    expect(html).toContain("반영을 기다리고 있어요");
    expect(html).not.toContain("ITSM에 다시 반영하기");
  });
});
