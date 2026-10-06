import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  TaskCompletionDialog,
  taskCompletionCopy,
} from "./TaskCompletionDialog";
import { mergeInflowDraftValues } from "./ProjectInflowPanel";
import { mergeGmailInflowDraftValues } from "./GmailInflowReview";
import type { Task } from "../api/planning";

describe("optional task completion replies", () => {
  it("keeps a manually selected type when Chat or Gmail analysis refreshes", () => {
    const chat = {
      title: "확인",
      notes: "확인할 자료",
      assigneeName: "김경주",
      priority: "1",
      dueAt: "",
      withoutDeadline: true,
      workKind: "verification" as const,
    };
    expect(
      mergeInflowDraftValues(chat, { ...chat, workKind: "general" }, [
        "workKind",
      ]).workKind,
    ).toBe("verification");
    const mail = {
      title: "개발",
      notes: "개발할 내용",
      assigneeName: "김경주",
      priority: 1,
      dueAt: "",
      workKind: "development" as const,
    };
    expect(
      mergeGmailInflowDraftValues(mail, { ...mail, workKind: "general" }, [
        "workKind",
      ]).workKind,
    ).toBe("development");
  });
  it("explains saving results and the conditional source reply before completing", () => {
    const task = {
      id: "task",
      title: "정산 금액 확인",
      workKind: "verification",
    } as Task;
    const markup = renderToStaticMarkup(
      createElement(TaskCompletionDialog, {
        task,
        async onComplete() {},
        onCancel() {},
      }),
    );
    expect(markup).toContain(task.title);
    expect(markup).toContain(taskCompletionCopy.label);
    expect(markup).toContain(taskCompletionCopy.complete);
    expect(markup).toContain('maxLength="2000"');
    expect(markup).toContain("원래 대화에도 전달");
    expect(markup).not.toContain("확인 업무");
    expect(markup).not.toContain("개발 업무");
    expect(markup).not.toContain("일반 업무");
  });
  it.each(["general", "verification", "development"] as const)(
    "offers reply-less completion for legacy %s tasks",
    (workKind) => {
      const markup = renderToStaticMarkup(
        createElement(TaskCompletionDialog, {
          task: { id: "task", title: "작업", workKind } as Task,
          async onComplete() {},
          onCancel() {},
        }),
      );
      expect(markup).toContain(taskCompletionCopy.complete);
      expect(markup).toContain(taskCompletionCopy.cancel);
      expect(markup).not.toContain('required=""');
      expect(markup.match(/type="submit"/g)).toHaveLength(1);
      expect(markup.match(/<button\b/g)).toHaveLength(3);
      expect(markup).not.toContain("답글 없이 완료하기");
      expect(markup).not.toContain("답글 남기고 완료하기");
    },
  );
});
