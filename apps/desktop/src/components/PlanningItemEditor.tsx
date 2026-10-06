import { TaskWorkKindSelect } from "./TaskWorkKind";
import type { TaskWorkKind } from "../api/planning";
import { CalendarClock, ListTodo, Trash2, X } from "lucide-react";
import { FormEvent, useEffect, useRef, useState, type ReactNode } from "react";

import { type ScheduleEntry, type Task } from "../api/planning";
import { copy } from "../copy";
import { deadlinePickerCopy } from "../copy/deadlinePicker";
import { registerMobileBackHandler } from "../mobileBack";
import {
  DeadlinePicker,
  isoToSeoulLocalDateTime,
  resolveOptionalSeoulDateTime,
  seoulLocalDateTimeToIso,
} from "./DeadlinePicker";
import {
  scheduleLinkageForTask,
  scheduleTaskOptionLabel,
  type ScheduleProjectReference,
} from "./scheduleLinkage";

export type PlanningEditTarget =
  { kind: "task"; item: Task } | { kind: "schedule"; item: ScheduleEntry };

type TaskEditInput = {
  title: string;
  notes?: string;
  assigneeName?: string;
  status: Task["status"];
  workKind?: TaskWorkKind;
  priority: number;
  dueAt?: string;
};

export type ScheduleEditInput = {
  title: string;
  notes?: string;
  startsAt: string;
  endsAt: string;
  linkage?: {
    projectId: string | null;
    taskId: string | null;
  };
};

type PlanningItemEditorProps = {
  target: PlanningEditTarget | undefined;
  linkableTasks?: Task[];
  projects?: ScheduleProjectReference[];
  onClose(): void;
  onSaveTask(task: Task, input: TaskEditInput): Promise<void>;
  onSaveSchedule(entry: ScheduleEntry, input: ScheduleEditInput): Promise<void>;
  onDeleteTask(task: Task): Promise<void>;
  onDeleteSchedule(entry: ScheduleEntry): Promise<void>;
};

export function PlanningItemEditor({
  target,
  linkableTasks = [],
  projects = [],
  onClose,
  onSaveTask,
  onSaveSchedule,
  onDeleteTask,
  onDeleteSchedule,
}: PlanningItemEditorProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const deleteTriggerRef = useRef<HTMLButtonElement>(null);
  const deleteSafeActionRef = useRef<HTMLButtonElement>(null);
  const restoreDeleteTriggerRef = useRef(false);
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [assigneeName, setAssigneeName] = useState("");
  const [priority, setPriority] = useState(1);
  const [workKind, setWorkKind] = useState<TaskWorkKind>("general");
  const [dueAt, setDueAt] = useState("");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [linkedTaskId, setLinkedTaskId] = useState("");
  const [saving, setSaving] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [error, setError] = useState<string>();
  const [confirmingDiscard, setConfirmingDiscard] = useState(false);
  const initializedTarget = useRef<string | undefined>(undefined);
  const baseline = useRef("");
  const closeRequestRef = useRef<() => void>(() => undefined);
  const draft = JSON.stringify([
    title,
    notes,
    assigneeName,
    priority,
    workKind,
    dueAt,
    startsAt,
    endsAt,
    linkedTaskId,
  ]);
  closeRequestRef.current = () => {
    if (saving) return;
    if (draft !== baseline.current) {
      setConfirmingDiscard(true);
      return;
    }
    dialogRef.current?.close();
  };

  useEffect(() => {
    if (!target) {
      initializedTarget.current = undefined;
      return;
    }
    const targetKey = `${target.kind}:${target.item.id}`;
    if (initializedTarget.current === targetKey) return;
    initializedTarget.current = targetKey;
    baseline.current = JSON.stringify([
      target.item.title,
      target.item.notes ?? "",
      target.kind === "task" ? (target.item.assigneeName ?? "") : "",
      target.kind === "task" ? target.item.priority : 1,
      target.kind === "task" ? (target.item.workKind ?? "general") : "general",
      target.kind === "task" ? isoToLocalInput(target.item.dueAt) : "",
      target.kind === "schedule" ? isoToLocalInput(target.item.startsAt) : "",
      target.kind === "schedule" ? isoToLocalInput(target.item.endsAt) : "",
      target.kind === "schedule" ? (target.item.taskId ?? "") : "",
    ]);
    setConfirmingDiscard(false);
    let focusFrame: number | undefined;
    openerRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    setTitle(target.item.title);
    setNotes(target.item.notes ?? "");
    setAssigneeName(
      target.kind === "task" ? (target.item.assigneeName ?? "") : "",
    );
    setPriority(target.kind === "task" ? target.item.priority : 1);
    setWorkKind(
      target.kind === "task" ? (target.item.workKind ?? "general") : "general",
    );
    setDueAt(target.kind === "task" ? isoToLocalInput(target.item.dueAt) : "");
    setStartsAt(
      target.kind === "schedule" ? isoToLocalInput(target.item.startsAt) : "",
    );
    setEndsAt(
      target.kind === "schedule" ? isoToLocalInput(target.item.endsAt) : "",
    );
    setLinkedTaskId(
      target.kind === "schedule" ? (target.item.taskId ?? "") : "",
    );
    setSaving(false);
    setConfirmingDelete(false);
    restoreDeleteTriggerRef.current = false;
    setError(undefined);
    const dialog = dialogRef.current;
    if (dialog && !dialog.open) {
      dialog.showModal();
      focusFrame = window.requestAnimationFrame(() => {
        titleInputRef.current?.focus();
      });
    }
    return () => {
      if (focusFrame !== undefined) window.cancelAnimationFrame(focusFrame);
    };
  }, [target]);

  useEffect(() => {
    const focusTarget = confirmingDelete
      ? deleteSafeActionRef.current
      : restoreDeleteTriggerRef.current
        ? deleteTriggerRef.current
        : undefined;
    if (!focusTarget) return;
    const frame = window.requestAnimationFrame(() => {
      focusTarget.focus();
      if (!confirmingDelete) restoreDeleteTriggerRef.current = false;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [confirmingDelete]);

  useEffect(() => {
    if (!target) return;
    return registerMobileBackHandler(() => {
      if (saving) return true;
      closeRequestRef.current();
      return true;
    }, 100);
  }, [saving, target]);

  if (!target) return null;
  const activeTarget = target;

  const taskTarget = activeTarget.kind === "task";
  const heading = taskTarget
    ? copy.forms.editTaskTitle
    : copy.forms.editScheduleTitle;
  const description = taskTarget
    ? copy.forms.editTaskDescription
    : copy.forms.editScheduleDescription;

  function requestClose() {
    closeRequestRef.current();
  }

  function handleClose() {
    openerRef.current?.focus();
    onClose();
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (saving) return;
    const nextTitle = title.trim();
    if (!nextTitle) {
      setError(copy.forms.titleRequired);
      return;
    }
    let taskDueAt: string | undefined;
    if (activeTarget.kind === "task") {
      const deadline = resolveOptionalSeoulDateTime(dueAt);
      if (!deadline.valid) {
        setError(deadlinePickerCopy.invalid);
        document.getElementById("planning-edit-due-at-date")?.focus();
        return;
      }
      taskDueAt = deadline.value;
    } else if (!startsAt || !endsAt) {
      setError(copy.forms.scheduleTimeRequired);
      return;
    } else {
      const start = seoulLocalDateTimeToIso(startsAt);
      const end = seoulLocalDateTimeToIso(endsAt);
      if (
        !start ||
        !end ||
        new Date(end).getTime() <= new Date(start).getTime()
      ) {
        setError(copy.forms.scheduleTimeOrder);
        return;
      }
    }
    setSaving(true);
    setError(undefined);
    try {
      if (activeTarget.kind === "task") {
        await onSaveTask(activeTarget.item, {
          title: nextTitle,
          notes: notes.trim() || undefined,
          assigneeName: assigneeName.trim() || undefined,
          status: activeTarget.item.status,
          priority,
          workKind,
          dueAt: taskDueAt,
        });
      } else {
        const start = seoulLocalDateTimeToIso(startsAt);
        const end = seoulLocalDateTimeToIso(endsAt);
        const originalTaskId = activeTarget.item.taskId ?? "";
        await onSaveSchedule(activeTarget.item, {
          title: nextTitle,
          notes: notes.trim() || undefined,
          startsAt: start!,
          endsAt: end!,
          ...(linkedTaskId === originalTaskId
            ? {}
            : {
                linkage: scheduleLinkageForTask(linkableTasks, linkedTaskId),
              }),
        });
      }
      dialogRef.current?.close();
    } catch {
      setError(
        activeTarget.kind === "task"
          ? copy.messages.taskSaveNotice
          : copy.messages.scheduleChanged,
      );
      setSaving(false);
    }
  }

  async function deleteItem() {
    if (saving) return;
    setSaving(true);
    setError(undefined);
    try {
      if (activeTarget.kind === "task") {
        await onDeleteTask(activeTarget.item);
      } else {
        await onDeleteSchedule(activeTarget.item);
      }
      dialogRef.current?.close();
    } catch {
      setError(
        activeTarget.kind === "task"
          ? copy.messages.taskDeleteNotice
          : copy.messages.scheduleDeleteNotice,
      );
      setSaving(false);
      restoreDeleteTriggerRef.current = true;
      setConfirmingDelete(false);
    }
  }

  return (
    <dialog
      ref={dialogRef}
      className="planning-editor"
      aria-labelledby="planning-editor-title"
      aria-describedby="planning-editor-description"
      onCancel={(event) => {
        event.preventDefault();
        requestClose();
      }}
      onKeyDown={(event) => {
        if (event.key !== "Escape") return;
        event.preventDefault();
        requestClose();
      }}
      onClose={handleClose}
    >
      <form aria-busy={saving} onSubmit={(event) => void submit(event)}>
        <header className="planning-editor__heading">
          <span aria-hidden="true">
            {taskTarget ? <ListTodo /> : <CalendarClock />}
          </span>
          <div>
            <h2 id="planning-editor-title">{heading}</h2>
            <p id="planning-editor-description">{description}</p>
          </div>
          <button
            className="planning-editor__close focus-visible-control"
            type="button"
            onClick={requestClose}
            disabled={saving}
            aria-label={copy.actions.cancel}
          >
            <X aria-hidden="true" />
          </button>
        </header>

        <fieldset disabled={saving}>
          <EditorField label={copy.forms.title} htmlFor="planning-edit-title">
            <input
              ref={titleInputRef}
              id="planning-edit-title"
              required
              maxLength={200}
              value={title}
              onChange={(event) => {
                setTitle(event.target.value);
                setError(undefined);
              }}
            />
          </EditorField>

          <EditorField label={copy.forms.notes} htmlFor="planning-edit-notes">
            <textarea
              id="planning-edit-notes"
              maxLength={10_000}
              rows={4}
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
            />
          </EditorField>

          {activeTarget.kind === "task" && activeTarget.item.completionNote && (
            <section className="task-completion-result">
              <strong>확인 결과</strong>
              <p>{activeTarget.item.completionNote}</p>
            </section>
          )}
          {activeTarget.kind === "task" ? (
            <>
              <EditorField
                label={copy.forms.assignee}
                htmlFor="planning-edit-assignee"
              >
                <input
                  id="planning-edit-assignee"
                  maxLength={80}
                  value={assigneeName}
                  placeholder={copy.forms.assigneePlaceholder}
                  onChange={(event) => {
                    setAssigneeName(event.target.value);
                    setError(undefined);
                  }}
                />
              </EditorField>
              <TaskWorkKindSelect
                value={workKind}
                disabled={saving}
                onChange={setWorkKind}
              />
              <div className="planning-editor__field-grid">
                <EditorField
                  label={copy.forms.priority}
                  htmlFor="planning-edit-priority"
                >
                  <select
                    id="planning-edit-priority"
                    value={priority}
                    onChange={(event) =>
                      setPriority(Number(event.target.value))
                    }
                  >
                    <option value={0}>{copy.forms.priorityNormal}</option>
                    <option value={1}>{copy.forms.prioritySoon}</option>
                    <option value={2}>{copy.forms.priorityImportant}</option>
                    <option value={3}>{copy.forms.priorityHighest}</option>
                  </select>
                </EditorField>
                <DeadlinePicker
                  className="planning-editor__field"
                  id="planning-edit-due-at"
                  label={copy.forms.dueAt}
                  value={dueAt}
                  disabled={saving}
                  showPresets
                  onChange={setDueAt}
                />
              </div>
            </>
          ) : (
            <>
              <div className="planning-editor__field-grid">
                <DeadlinePicker
                  className="planning-editor__field"
                  id="planning-edit-starts-at"
                  label={copy.forms.startsAt}
                  value={startsAt}
                  disabled={saving}
                  required
                  allowClear={false}
                  onChange={(value) => {
                    setStartsAt(value);
                    setError(undefined);
                  }}
                />
                <DeadlinePicker
                  className="planning-editor__field"
                  id="planning-edit-ends-at"
                  label={copy.forms.endsAt}
                  value={endsAt}
                  disabled={saving}
                  required
                  allowClear={false}
                  onChange={(value) => {
                    setEndsAt(value);
                    setError(undefined);
                  }}
                />
              </div>
              {activeTarget.item.source === "manual" && (
                <EditorField
                  label={copy.forms.linkedTask}
                  htmlFor="planning-edit-linked-task"
                  description={copy.forms.linkedTaskDescription}
                >
                  <select
                    id="planning-edit-linked-task"
                    aria-describedby="planning-edit-linked-task-description"
                    value={linkedTaskId}
                    onChange={(event) => setLinkedTaskId(event.target.value)}
                  >
                    <option value="">{copy.forms.linkedTaskNone}</option>
                    {linkableTasks.map((task) => (
                      <option key={task.id} value={task.id}>
                        {scheduleTaskOptionLabel(task, projects, {
                          noProject: copy.forms.linkedTaskNoProject,
                          unknownProject: copy.forms.linkedTaskUnknownProject,
                          unassigned: copy.home.unassignedTaskGroup,
                          noDueDate: copy.home.noDueDateTaskGroup,
                        })}
                      </option>
                    ))}
                  </select>
                </EditorField>
              )}
            </>
          )}
        </fieldset>

        {error && (
          <p className="planning-editor__error" role="alert">
            {error}
          </p>
        )}

        {confirmingDiscard ? (
          <section
            className="planning-editor__discard-confirmation"
            role="group"
            aria-label="변경 내용 확인"
          >
            <p>
              저장하지 않은 변경 내용이 있어요. 닫으면 변경 내용이 사라져요.
            </p>
            <div>
              <button
                className="secondary-button focus-visible-control"
                type="button"
                autoFocus
                onClick={() => {
                  setConfirmingDiscard(false);
                  titleInputRef.current?.focus();
                }}
              >
                계속 수정하기
              </button>
              <button
                className="danger-button focus-visible-control"
                type="button"
                onClick={() => dialogRef.current?.close()}
              >
                저장하지 않고 닫기
              </button>
            </div>
          </section>
        ) : confirmingDelete ? (
          <section
            className="planning-editor__delete-confirmation"
            role="group"
            aria-label={
              taskTarget
                ? copy.forms.deleteTaskTitle
                : copy.forms.deleteScheduleTitle
            }
          >
            <div>
              <strong>
                {taskTarget
                  ? copy.forms.deleteTaskTitle
                  : copy.forms.deleteScheduleTitle}
              </strong>
              <p>
                {taskTarget
                  ? copy.forms.deleteTaskDescription
                  : copy.forms.deleteScheduleDescription}
              </p>
            </div>
            <div>
              <button
                ref={deleteSafeActionRef}
                className="secondary-button focus-visible-control"
                type="button"
                onClick={() => {
                  restoreDeleteTriggerRef.current = true;
                  setConfirmingDelete(false);
                }}
                disabled={saving}
              >
                {taskTarget ? copy.actions.keepTask : copy.actions.keepSchedule}
              </button>
              <button
                className="danger-button focus-visible-control"
                type="button"
                onClick={() => void deleteItem()}
                disabled={saving}
              >
                {saving ? (
                  <span className="button-spinner" aria-hidden="true" />
                ) : (
                  <Trash2 aria-hidden="true" />
                )}
                {saving
                  ? copy.actions.deleting
                  : taskTarget
                    ? copy.actions.deleteTask
                    : copy.actions.deleteSchedule}
              </button>
            </div>
          </section>
        ) : (
          <footer className="planning-editor__actions">
            <span className="planning-editor__action-spacer" />
            <button
              ref={deleteTriggerRef}
              className="secondary-button focus-visible-control"
              type="button"
              onClick={() => setConfirmingDelete(true)}
              disabled={saving}
            >
              {copy.actions.deleteContent}
            </button>
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={saving || !title.trim()}
            >
              {saving ? (
                <span className="button-spinner" aria-hidden="true" />
              ) : null}
              {saving ? copy.actions.saving : copy.actions.saveChanges}
            </button>
          </footer>
        )}
      </form>
    </dialog>
  );
}

function EditorField({
  label,
  htmlFor,
  description,
  children,
}: {
  label: string;
  htmlFor: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="planning-editor__field">
      <label htmlFor={htmlFor}>{label}</label>
      {children}
      {description && <p id={`${htmlFor}-description`}>{description}</p>}
    </div>
  );
}

function isoToLocalInput(value: string | null): string {
  return isoToSeoulLocalDateTime(value);
}
