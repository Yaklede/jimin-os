import { useEffect, useRef, useState } from "react";
import { FilePenLine, X } from "lucide-react";
import type { ProjectInflowItem } from "../api/googleChat";
import type { Task } from "../api/planning";
import { registerMobileBackHandler } from "../mobileBack";
import {
  DeadlinePicker,
  isoToSeoulLocalDateTime,
  resolveOptionalSeoulDateTime,
} from "./DeadlinePicker";
import { LinkifiedText } from "./ExternalTextLink";
export function InflowChangeDialog({
  item,
  onLoad,
  onApprove,
  onClose,
}: {
  item: ProjectInflowItem;
  onLoad(id: string): Promise<Task>;
  onApprove(task: Task, notes: string, dueAt: string | null): Promise<void>;
  onClose(): void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const keepReviewing = useRef<HTMLButtonElement>(null);
  const [task, setTask] = useState<Task>();
  const [notes, setNotes] = useState("");
  const [dueAt, setDueAt] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [discard, setDiscard] = useState(false);
  const baseline = useRef("");
  const closeRef = useRef(() => {});
  closeRef.current = () => {
    if (saving) return;
    if (JSON.stringify([notes, dueAt]) !== baseline.current && task) {
      setDiscard(true);
      return;
    }
    dialog.current?.close();
  };
  useEffect(() => {
    let active = true;
    opener.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    dialog.current?.showModal();
    dialog.current?.querySelector<HTMLButtonElement>("button")?.focus();
    void onLoad(item.promotedTaskId!)
      .then((value) => {
        if (!active) return;
        setTask(value);
        const nextNotes = [
          value.notes,
          item.changeActionItems?.length
            ? `추가 요청\n${item.changeActionItems.join("\n")}`
            : "",
        ]
          .filter(Boolean)
          .join("\n\n");
        const nextDue = isoToSeoulLocalDateTime(
          item.suggestedDueAt ?? value.dueAt,
        );
        setNotes(nextNotes);
        setDueAt(nextDue);
        baseline.current = JSON.stringify([nextNotes, nextDue]);
      })
      .catch(() => {
        if (active)
          setError("일감을 불러오지 못했어요. 창을 닫고 다시 확인해 주세요.");
      });
    return () => {
      active = false;
      opener.current?.focus({ preventScroll: true });
    };
  }, [item.id]);
  useEffect(
    () =>
      registerMobileBackHandler(() => {
        closeRef.current();
        return true;
      }, 110),
    [],
  );
  useEffect(() => {
    if (discard) keepReviewing.current?.focus({ preventScroll: true });
  }, [discard]);
  return (
    <dialog
      ref={dialog}
      className="planning-editor"
      aria-labelledby="inflow-change-title"
      aria-describedby="inflow-change-description"
      aria-busy={saving}
      onClose={onClose}
      onCancel={(event) => {
        event.preventDefault();
        closeRef.current();
      }}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!task || task.status !== "open" || saving) return;
          const due = resolveOptionalSeoulDateTime(dueAt);
          if (!due.valid) {
            setError("마감 날짜와 시간을 확인해 주세요.");
            return;
          }
          setSaving(true);
          setError("");
          void onApprove(task, notes, due.value ?? null)
            .then(() => dialog.current?.close())
            .catch(() =>
              setError(
                "검토 중 새 답글이나 일감 변경이 생겼을 수 있어요. 창을 닫고 최신 제안을 확인한 뒤 다시 시도해 주세요.",
              ),
            )
            .finally(() => setSaving(false));
        }}
      >
        <header className="planning-editor__heading">
          <span aria-hidden="true">
            <FilePenLine />
          </span>
          <div>
            <h2 id="inflow-change-title">기존 일감 변경 검토</h2>
            <p id="inflow-change-description">
              새 일감을 만들지 않아요. 확인한 내용과 마감일만 기존 일감에
              반영해요.
            </p>
          </div>
          <button
            type="button"
            className="planning-editor__close focus-visible-control"
            aria-label="변경 검토 닫기"
            disabled={saving}
            onClick={() => closeRef.current()}
          >
            <X />
          </button>
        </header>
        <fieldset disabled={saving}>
          {task ? (
            <>
              <strong>{task.title}</strong>
              <details>
                <summary>현재 내용 보기</summary>
                <div>
                  <LinkifiedText text={task.notes ?? "설명이 없어요."} />
                </div>
                <p>
                  현재 마감:{" "}
                  {task.dueAt
                    ? new Date(task.dueAt).toLocaleString("ko-KR")
                    : "기한 없음"}
                </p>
              </details>
              <label className="planning-editor__field">
                반영할 내용
                <textarea
                  rows={6}
                  maxLength={10000}
                  disabled={saving}
                  value={notes}
                  onChange={(event) => setNotes(event.target.value)}
                />
              </label>
              <DeadlinePicker
                id="inflow-change-due"
                className="planning-editor__field"
                label="반영할 마감일"
                value={dueAt}
                disabled={saving}
                showPresets
                onChange={setDueAt}
              />
            </>
          ) : !error ? (
            <p role="status">일감을 확인하고 있어요.</p>
          ) : null}
          {task && task.status !== "open" && (
            <p role="status">
              이미 완료하거나 취소한 일감이에요. 다시 열어야 변경을 반영할 수
              있어요.
            </p>
          )}
        </fieldset>
        {error && (
          <p role="alert" className="planning-editor__error">
            {error}
          </p>
        )}
        {discard ? (
          <div role="alert" className="planning-editor__discard-confirmation">
            <p>작성한 변경 내용을 버리고 닫을까요?</p>
            <div>
              <button
                ref={keepReviewing}
                type="button"
                className="secondary-button"
                onClick={() => setDiscard(false)}
              >
                계속 검토하기
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => dialog.current?.close()}
              >
                버리고 닫기
              </button>
            </div>
          </div>
        ) : (
          <footer className="planning-editor__actions">
            <button
              type="button"
              className="secondary-button"
              disabled={saving}
              onClick={() => closeRef.current()}
            >
              취소
            </button>
            <button
              type="submit"
              className="primary-button"
              disabled={saving || !task || task.status !== "open"}
            >
              {saving ? "반영 중" : "확인한 변경 반영하기"}
            </button>
          </footer>
        )}
      </form>
    </dialog>
  );
}
