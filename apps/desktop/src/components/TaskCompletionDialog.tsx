import { useEffect, useRef, useState } from "react";
import { PlanningRequestError, type Task } from "../api/planning";
import { LinkifiedText } from "./ExternalTextLink";
import {
  InflowPromotionDialog,
  inflowPromotionDialogCopy,
} from "./InflowPromotionDialog";

export const taskCompletionCopy = {
  ...inflowPromotionDialogCopy,
  title: "확인 결과 남기기",
  description:
    "결과를 일감에 저장해요. Google Chat에서 받은 일감이면 원래 대화에도 답글을 남겨요.",
  close: "완료 화면 닫기",
  discardTitle: "확인 결과 작성을 취소할까요?",
  discardDescription: "작성한 결과는 저장되지 않고, 일감은 완료하지 않아요.",
  label: "확인 결과",
  placeholder: "예: 거래내역 20건을 확인했고, 정산 금액이 모두 일치해요.",
  save: "결과를 남기고 완료하기",
  skip: "결과 없이 완료하기",
  cancel: "취소",
  busy: "결과를 저장하고 있어요",
  failure:
    "완료하지 못했어요. 작성한 결과는 그대로 있어요. 다시 시도해 주세요.",
  conflict:
    "다른 곳에서 일감이 변경됐어요. 작성한 결과를 복사한 뒤 창을 닫고, 최신 일감을 다시 열어 주세요.",
  required: "확인 결과를 적거나, 결과 없이 완료하기를 눌러 주세요.",
};

export function TaskCompletionDialog({
  task,
  onComplete,
  onCancel,
}: {
  task: Task;
  onComplete(note?: string): Promise<void>;
  onCancel(): void;
}) {
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const inFlight = useRef(false);
  async function finish(withResult: boolean) {
    if (inFlight.current) return;
    if (withResult && !note.trim()) {
      setError(taskCompletionCopy.required);
      return;
    }
    inFlight.current = true;
    setBusy(true);
    setError(undefined);
    try {
      await onComplete(withResult ? note.trim() : undefined);
    } catch (cause) {
      setError(
        cause instanceof PlanningRequestError && cause.code === "conflict"
          ? taskCompletionCopy.conflict
          : taskCompletionCopy.failure,
      );
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <InflowPromotionDialog
      open
      busy={busy}
      dirty={Boolean(note.trim())}
      onClose={onCancel}
      dialogCopy={taskCompletionCopy}
      className="task-completion-dialog"
    >
      {(requestClose) => (
        <form
          className="task-completion-form"
          onSubmit={(event) => {
            event.preventDefault();
            void finish(true);
          }}
        >
          <strong>
            <LinkifiedText text={task.title} />
          </strong>
          <label>
            <span>{taskCompletionCopy.label}</span>
            <textarea
              aria-label={taskCompletionCopy.label}
              value={note}
              maxLength={2000}
              rows={5}
              placeholder={taskCompletionCopy.placeholder}
              disabled={busy}
              onChange={(event) => {
                setNote(event.target.value);
                setError(undefined);
              }}
            />
          </label>
          <small>{note.length} / 2,000</small>
          {error && <p role="alert">{error}</p>}
          {busy && <p role="status">{taskCompletionCopy.busy}</p>}
          <div className="task-completion-form__actions">
            <button
              type="button"
              className="secondary-button"
              disabled={busy}
              onClick={requestClose}
            >
              {taskCompletionCopy.cancel}
            </button>
            <button
              type="button"
              className="secondary-button"
              disabled={busy}
              onClick={() => void finish(false)}
            >
              {taskCompletionCopy.skip}
            </button>
            <button type="submit" className="primary-button" disabled={busy}>
              {taskCompletionCopy.save}
            </button>
          </div>
        </form>
      )}
    </InflowPromotionDialog>
  );
}

// Every completion entry point shares this modal and the same version-fenced request.
// Cancelling resolves with the unchanged task, so callers must not remove it.
export function useTaskCompletion(
  complete: (task: Task, note?: string) => Promise<Task>,
) {
  const [pending, setPending] = useState<Task>();
  const request = useRef<
    | { task: Task; promise: Promise<Task>; resolve(task: Task): void }
    | undefined
  >(undefined);
  useEffect(
    () => () => {
      request.current?.resolve(request.current.task);
      request.current = undefined;
    },
    [],
  );
  function requestCompletion(task: Task): Promise<Task> {
    if (request.current)
      return request.current.task.id === task.id
        ? request.current.promise
        : Promise.resolve(task);
    if (task.workKind !== "verification") return complete(task);
    let resolve!: (value: Task) => void;
    const promise = new Promise<Task>((accept) => {
      resolve = accept;
    });
    request.current = { task, promise, resolve };
    setPending(task);
    return promise;
  }
  function finish(task: Task) {
    request.current?.resolve(task);
    request.current = undefined;
    setPending(undefined);
  }
  const completionDialog = pending ? (
    <TaskCompletionDialog
      key={pending.id}
      task={pending}
      onCancel={() => finish(pending)}
      onComplete={async (note) => {
        const completed = await complete(pending, note);
        finish(completed);
      }}
    />
  ) : null;
  return { requestCompletion, completionDialog };
}
