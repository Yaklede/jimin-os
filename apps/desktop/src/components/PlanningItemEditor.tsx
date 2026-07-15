import { CalendarClock, ListTodo, Trash2, X } from "lucide-react";
import { FormEvent, useEffect, useRef, useState, type ReactNode } from "react";

import { type ScheduleEntry, type Task } from "../api/planning";
import { copy } from "../copy";

export type PlanningEditTarget =
  { kind: "task"; item: Task } | { kind: "schedule"; item: ScheduleEntry };

type TaskEditInput = {
  title: string;
  notes?: string;
  status: Task["status"];
  priority: number;
  dueAt?: string;
};

type ScheduleEditInput = {
  title: string;
  notes?: string;
  startsAt: string;
  endsAt: string;
};

type PlanningItemEditorProps = {
  target: PlanningEditTarget | undefined;
  onClose(): void;
  onSaveTask(task: Task, input: TaskEditInput): Promise<void>;
  onSaveSchedule(entry: ScheduleEntry, input: ScheduleEditInput): Promise<void>;
  onDeleteTask(task: Task): Promise<void>;
  onDeleteSchedule(entry: ScheduleEntry): Promise<void>;
};

export function PlanningItemEditor({
  target,
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
  const [priority, setPriority] = useState(1);
  const [dueAt, setDueAt] = useState("");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [saving, setSaving] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!target) return;
    let focusFrame: number | undefined;
    openerRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    setTitle(target.item.title);
    setNotes(target.item.notes ?? "");
    setPriority(target.kind === "task" ? target.item.priority : 1);
    setDueAt(target.kind === "task" ? isoToLocalInput(target.item.dueAt) : "");
    setStartsAt(
      target.kind === "schedule" ? isoToLocalInput(target.item.startsAt) : "",
    );
    setEndsAt(
      target.kind === "schedule" ? isoToLocalInput(target.item.endsAt) : "",
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
    if (saving) return;
    dialogRef.current?.close();
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
    setSaving(true);
    setError(undefined);
    try {
      if (activeTarget.kind === "task") {
        await onSaveTask(activeTarget.item, {
          title: nextTitle,
          notes: notes.trim() || undefined,
          status: activeTarget.item.status,
          priority,
          dueAt: dueAt ? localInputToIso(dueAt) : undefined,
        });
      } else {
        if (!startsAt || !endsAt) {
          setError(copy.forms.scheduleTimeRequired);
          setSaving(false);
          return;
        }
        const start = new Date(startsAt);
        const end = new Date(endsAt);
        if (
          Number.isNaN(start.getTime()) ||
          Number.isNaN(end.getTime()) ||
          end <= start
        ) {
          setError(copy.forms.scheduleTimeOrder);
          setSaving(false);
          return;
        }
        await onSaveSchedule(activeTarget.item, {
          title: nextTitle,
          notes: notes.trim() || undefined,
          startsAt: start.toISOString(),
          endsAt: end.toISOString(),
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

          {activeTarget.kind === "task" ? (
            <div className="planning-editor__field-grid">
              <EditorField
                label={copy.forms.priority}
                htmlFor="planning-edit-priority"
              >
                <select
                  id="planning-edit-priority"
                  value={priority}
                  onChange={(event) => setPriority(Number(event.target.value))}
                >
                  <option value={0}>{copy.forms.priorityNormal}</option>
                  <option value={1}>{copy.forms.prioritySoon}</option>
                  <option value={2}>{copy.forms.priorityImportant}</option>
                  <option value={3}>{copy.forms.priorityHighest}</option>
                </select>
              </EditorField>
              <EditorField
                label={copy.forms.dueAt}
                htmlFor="planning-edit-due-at"
                description={copy.forms.dueAtDescription}
              >
                <input
                  id="planning-edit-due-at"
                  type="datetime-local"
                  value={dueAt}
                  onInput={(event) => setDueAt(event.currentTarget.value)}
                />
              </EditorField>
            </div>
          ) : (
            <div className="planning-editor__field-grid">
              <EditorField
                label={copy.forms.startsAt}
                htmlFor="planning-edit-starts-at"
              >
                <input
                  id="planning-edit-starts-at"
                  type="datetime-local"
                  required
                  value={startsAt}
                  onInput={(event) => {
                    setStartsAt(event.currentTarget.value);
                    setError(undefined);
                  }}
                />
              </EditorField>
              <EditorField
                label={copy.forms.endsAt}
                htmlFor="planning-edit-ends-at"
              >
                <input
                  id="planning-edit-ends-at"
                  type="datetime-local"
                  required
                  value={endsAt}
                  onInput={(event) => {
                    setEndsAt(event.currentTarget.value);
                    setError(undefined);
                  }}
                />
              </EditorField>
            </div>
          )}
        </fieldset>

        {error && (
          <p className="planning-editor__error" role="alert">
            {error}
          </p>
        )}

        {confirmingDelete ? (
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
            <button
              ref={deleteTriggerRef}
              className="text-button text-button--danger focus-visible-control"
              type="button"
              onClick={() => setConfirmingDelete(true)}
              disabled={saving}
            >
              <Trash2 aria-hidden="true" />
              {taskTarget
                ? copy.actions.deleteTask
                : copy.actions.deleteSchedule}
            </button>
            <span className="planning-editor__action-spacer" />
            <button
              className="secondary-button focus-visible-control"
              type="button"
              onClick={requestClose}
              disabled={saving}
            >
              {copy.actions.cancel}
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
      {description && <p>{description}</p>}
    </div>
  );
}

function isoToLocalInput(value: string | null): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

function localInputToIso(value: string): string {
  return new Date(value).toISOString();
}
