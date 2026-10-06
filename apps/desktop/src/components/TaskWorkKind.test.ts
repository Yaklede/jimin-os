import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TaskWorkKindSelect, TaskWorkKindBadge } from "./TaskWorkKind";
import {
  TaskCompletionDialog,
  taskCompletionCopy,
} from "./TaskCompletionDialog";
import { mergeInflowDraftValues } from "./ProjectInflowPanel";
import { mergeGmailInflowDraftValues } from "./GmailInflowReview";
import type { Task } from "../api/planning";

describe("task work type and completion result", () => {
  it("offers explicit verification/development choices and keeps legacy general work", () => {
    const markup = renderToStaticMarkup(
      createElement(TaskWorkKindSelect, {
        value: "verification",
        onChange() {},
      }),
    );
    expect(markup).toContain('value="verification" selected');
    expect(markup).toContain("확인 업무");
    expect(markup).toContain("개발 업무");
    expect(markup).toContain("일반 업무");
    expect(
      renderToStaticMarkup(
        createElement(TaskWorkKindBadge, { kind: "general" }),
      ),
    ).toBe("");
    expect(
      renderToStaticMarkup(
        createElement(TaskWorkKindBadge, { kind: "development" }),
      ),
    ).toContain("개발 업무");
  });
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
    expect(markup).toContain(taskCompletionCopy.save);
    expect(markup).toContain(taskCompletionCopy.skip);
    expect(markup).toContain('maxLength="2000"');
    expect(markup).toContain("원래 대화에도 답글");
  });
});
