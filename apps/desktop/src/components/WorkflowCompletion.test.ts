import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DeliveryRecoveryPanel } from "./DeliveryRecoveryPanel";
import { ReportEvidencePanel } from "./ReportEvidencePanel";
import { InflowChangeDialog } from "./InflowChangeDialog";
import type { ProjectInflowItem } from "../api/googleChat";
import type { DeliveryRecovery } from "../api/deliveryRecovery";
describe("workflow review surfaces", () => {
  it("offers retries only for eligible failed receipts, not automatic retries or expired push", () => {
    const base: DeliveryRecovery = {
      id: "a",
      kind: "push",
      status: "failed",
      canRetry: true,
      title: "테스트",
      projectId: null,
      attemptCount: 3,
      createdAt: "2026-10-08T00:00:00Z",
    };
    const markup = renderToStaticMarkup(
      createElement(DeliveryRecoveryPanel, {
        items: [
          base,
          { ...base, id: "b", canRetry: false },
          { ...base, id: "c", status: "retry_wait", canRetry: false },
        ],
        onRetry: async () => {},
      }),
    );
    expect(markup.match(/이 알림만 다시 보내기/g)).toHaveLength(1);
    expect(markup).toContain("자동으로 다시 시도하고 있어요");
    expect(markup).toContain("성공한 원문 표시·답글은 다시 보내지 않아요");
    expect(markup).not.toContain('<details open=""');
    expect(markup).toContain('<summary class="focus-visible-control">');
    expect(markup).toContain("lucide-chevron-down");
  });
  it("links evidence to its matching metric and explains current state and leaf-task counts", () => {
    const markup = renderToStaticMarkup(
      createElement(ReportEvidencePanel, {
        metric: "waiting",
        onLoad: async () => ({ items: [], nextCursor: null }),
        onOpen() {},
        onClose() {},
      }),
    );
    expect(markup).toContain('aria-label="응답을 기다리는 일"');
    expect(markup).toContain("하위 일감만 집계해요");
    expect(markup).toContain("현재 상태예요");
  });
  it("never enables approval before the current task has loaded", () => {
    const markup = renderToStaticMarkup(
      createElement(InflowChangeDialog, {
        item: { id: "reply", promotedTaskId: "task" } as ProjectInflowItem,
        onLoad: async () => {
          throw new Error("not loaded");
        },
        onApprove: async () => {},
        onClose() {},
      }),
    );
    expect(markup).toContain("새 일감을 만들지 않아요");
    expect(markup).toMatch(/<button[^>]*type="submit"[^>]*disabled/);
    expect(markup).toContain("일감을 확인하고 있어요");
    expect(markup).toMatch(
      /<header[^>]*planning-editor__heading[^>]*><span aria-hidden="true">/,
    );
    expect(markup).toContain("<fieldset>");
    expect(markup).toContain('aria-describedby="inflow-change-description"');
  });
});
