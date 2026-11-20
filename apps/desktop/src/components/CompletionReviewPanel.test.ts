import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import {
  canConfirmTogether,
  CompletionReviewCard,
  sameReviewVersion,
} from "./CompletionReviewPanel";
import type { CompletionReview } from "../api/completionReviews";
const review: CompletionReview = {
  id: "review",
  taskId: "task",
  projectId: null,
  projectTitle: "비스킷링크",
  title: "권한 오류 수정",
  taskNotes: "원래 요청한 처리 내용",
  assigneeName: "김경주",
  dueAt: null,
  reporterName: "김경주",
  completionNote: "브라우저 두 곳에서 검증했어요.",
  reportedTaskVersion: 2,
  taskVersion: 2,
  version: 1,
  createdAt: "2026-10-27T00:00:00Z",
};
it("keeps reports compact until opened", () => {
  const html = renderToStaticMarkup(
    createElement(CompletionReviewCard, {
      review,
      busy: false,
      saving: false,
      initiallyOpen: false,
      onDecide: async () => {},
    }),
  );
  expect(html).toContain('aria-expanded="false"');
  expect(html).not.toContain("완료 확인</button>");
});
it("shows evidence and separate owner decisions; rework requires a reason", () => {
  const html = renderToStaticMarkup(
    createElement(CompletionReviewCard, {
      review: { ...review, taskVersion: 3 },
      busy: false,
      saving: false,
      initiallyOpen: true,
      onDecide: async () => {},
    }),
  );
  expect(html).toContain(review.completionNote);
  expect(html).toContain("현재 일감 내용");
  expect(html).toContain("완료 보고 이후 일감이 바뀌었어요");
  expect(html).toMatch(/disabled=""[^>]*>재작업 요청/);
  expect(html).toContain("완료 확인</button>");
});

it("excludes changed tasks from bulk approval without hiding the individual review", () => {
  const changed = { ...review, taskVersion: 3 };
  expect(canConfirmTogether(review)).toBe(true);
  expect(canConfirmTogether(changed)).toBe(false);
  expect(sameReviewVersion(review, changed)).toBe(false);
  expect(sameReviewVersion(review, { ...review, version: 2 })).toBe(false);
  const html = renderToStaticMarkup(
    createElement(CompletionReviewCard, {
      review: changed,
      busy: false,
      saving: false,
      initiallyOpen: true,
      picked: false,
      onPick: () => {},
      onDecide: async () => {},
    }),
  );
  expect(html).toMatch(/type="checkbox"[^>]*disabled=""/);
  expect(html).toContain(
    "보고 이후 바뀐 일감은 내용을 열어 따로 확인해 주세요.",
  );
  expect(html).toContain("완료 확인</button>");
});
