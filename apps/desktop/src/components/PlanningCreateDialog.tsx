import { CalendarPlus, ListTodo, X } from "lucide-react";
import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";

import { copy } from "../copy";
import { deadlinePickerCopy } from "../copy/deadlinePicker";
import { registerMobileBackHandler } from "../mobileBack";
import {
  DeadlinePicker,
  isoToSeoulLocalDateTime,
  resolveOptionalSeoulDateTime,
  seoulLocalDateTimeToIso,
} from "./DeadlinePicker";
import { type Task } from "../api/planning";
import {
  scheduleLinkageForTask,
  scheduleTaskOptionLabel,
  type ScheduleProjectReference,
} from "./scheduleLinkage";

export type PlanningCreateKind = "task" | "schedule";

export type PlanningTaskCreateInput = {
  title: string;
  notes?: string;
  priority: number;
  dueAt?: string;
};

export type PlanningScheduleCreateInput = {
  title: string;
  notes?: string;
  startsAt: string;
  endsAt: string;
  projectId?: string | null;
  taskId?: string | null;
};

type PlanningCreateDialogProps = {
  kind: PlanningCreateKind | undefined;
  linkableTasks?: Task[];
  projects?: ScheduleProjectReference[];
  onClose(): void;
  onCreateTask(input: PlanningTaskCreateInput): Promise<void>;
  onCreateSchedule(input: PlanningScheduleCreateInput): Promise<void>;
};

export function PlanningCreateDialog({
  kind,
  linkableTasks = [],
  projects = [],
  onClose,
  onCreateTask,
  onCreateSchedule,
}: PlanningCreateDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [priority, setPriority] = useState(1);
  const [dueAt, setDueAt] = useState("");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [linkedTaskId, setLinkedTaskId] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!kind) return;
    const scheduleRange = defaultScheduleRange();
    openerRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    setTitle("");
    setNotes("");
    setPriority(1);
    setDueAt("");
    setStartsAt(scheduleRange.startsAt);
    setEndsAt(scheduleRange.endsAt);
    setLinkedTaskId("");
    setSaving(false);
    setError(undefined);

    const dialog = dialogRef.current;
    let focusFrame: number | undefined;
    if (dialog && !dialog.open) {
      dialog.showModal();
      focusFrame = window.requestAnimationFrame(() => {
        titleInputRef.current?.focus();
      });
    }
    return () => {
      if (focusFrame !== undefined) window.cancelAnimationFrame(focusFrame);
    };
  }, [kind]);

  useEffect(() => {
    if (!kind) return;
    return registerMobileBackHandler(() => {
      if (saving) return true;
      dialogRef.current?.close();
      return true;
    }, 100);
  }, [kind, saving]);

  if (!kind) return null;

  const taskMode = kind === "task";
  const heading = taskMode ? copy.forms.taskTitle : copy.forms.scheduleTitle;
  const description = taskMode
    ? copy.forms.taskCreateDescription
    : copy.forms.scheduleCreateDescription;

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
      titleInputRef.current?.focus();
      return;
    }
    let taskDueAt: string | undefined;
    if (taskMode) {
      const deadline = resolveOptionalSeoulDateTime(dueAt);
      if (!deadline.valid) {
        setError(deadlinePickerCopy.invalid);
        document.getElementById("planning-create-due-at-date")?.focus();
        return;
      }
      taskDueAt = deadline.value;
    } else {
      const scheduleError = validateScheduleTimes(startsAt, endsAt);
      if (scheduleError) {
        setError(scheduleError);
        return;
      }
    }

    setSaving(true);
    setError(undefined);
    try {
      if (taskMode) {
        await onCreateTask({
          title: nextTitle,
          notes: notes.trim() || undefined,
          priority,
          dueAt: taskDueAt,
        });
      } else {
        const linkage = scheduleLinkageForTask(linkableTasks, linkedTaskId);
        await onCreateSchedule({
          title: nextTitle,
          notes: notes.trim() || undefined,
          startsAt: localInputToIso(startsAt),
          endsAt: localInputToIso(endsAt),
          ...linkage,
        });
      }
      dialogRef.current?.close();
    } catch {
      setError(
        taskMode
          ? copy.messages.taskCreateNotice
          : copy.messages.scheduleCreateNotice,
      );
      setSaving(false);
    }
  }

  return (
    <dialog
      ref={dialogRef}
      className="planning-editor planning-create-dialog"
      aria-labelledby="planning-create-title"
      aria-describedby="planning-create-description"
      aria-busy={saving}
      onCancel={(event) => {
        event.preventDefault();
        requestClose();
      }}
      onClose={handleClose}
    >
      <form onSubmit={(event) => void submit(event)}>
        <header className="planning-editor__heading">
          <span aria-hidden="true">
            {taskMode ? <ListTodo /> : <CalendarPlus />}
          </span>
          <div>
            <h2 id="planning-create-title">{heading}</h2>
            <p id="planning-create-description">{description}</p>
          </div>
          <button
            className="planning-editor__close focus-visible-control"
            type="button"
            onClick={requestClose}
            disabled={saving}
            aria-label={copy.forms.closeCreateDialog(heading)}
          >
            <X aria-hidden="true" />
          </button>
        </header>

        <fieldset disabled={saving}>
          <CreateField label={copy.forms.title} htmlFor="planning-create-name">
            <input
              ref={titleInputRef}
              id="planning-create-name"
              required
              maxLength={200}
              value={title}
              aria-invalid={Boolean(error && !title.trim())}
              aria-describedby={error ? "planning-create-error" : undefined}
              onChange={(event) => {
                setTitle(event.target.value);
                setError(undefined);
              }}
            />
          </CreateField>

          <CreateField label={copy.forms.notes} htmlFor="planning-create-notes">
            <textarea
              id="planning-create-notes"
              maxLength={10_000}
              rows={4}
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
            />
          </CreateField>

          {taskMode ? (
            <div className="planning-editor__field-grid">
              <CreateField
                label={copy.forms.priority}
                htmlFor="planning-create-priority"
              >
                <select
                  id="planning-create-priority"
                  value={priority}
                  onChange={(event) => setPriority(Number(event.target.value))}
                >
                  <option value={0}>{copy.forms.priorityNormal}</option>
                  <option value={1}>{copy.forms.prioritySoon}</option>
                  <option value={2}>{copy.forms.priorityImportant}</option>
                  <option value={3}>{copy.forms.priorityHighest}</option>
                </select>
              </CreateField>
              <DeadlinePicker
                className="planning-editor__field"
                id="planning-create-due-at"
                label={copy.forms.dueAt}
                value={dueAt}
                disabled={saving}
                showPresets
                onChange={setDueAt}
              />
            </div>
          ) : (
            <>
              <div className="planning-editor__field-grid">
                <DeadlinePicker
                  className="planning-editor__field"
                  id="planning-create-starts-at"
                  label={copy.forms.startsAt}
                  value={startsAt}
                  disabled={saving}
                  required
                  allowClear={false}
                  describedBy={error ? "planning-create-error" : undefined}
                  onChange={(value) => {
                    setStartsAt(value);
                    setError(undefined);
                  }}
                />
                <DeadlinePicker
                  className="planning-editor__field"
                  id="planning-create-ends-at"
                  label={copy.forms.endsAt}
                  value={endsAt}
                  disabled={saving}
                  required
                  allowClear={false}
                  describedBy={error ? "planning-create-error" : undefined}
                  onChange={(value) => {
                    setEndsAt(value);
                    setError(undefined);
                  }}
                />
              </div>
              <CreateField
                label={copy.forms.linkedTask}
                htmlFor="planning-create-linked-task"
                description={copy.forms.linkedTaskDescription}
              >
                <select
                  id="planning-create-linked-task"
                  aria-describedby="planning-create-linked-task-description"
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
              </CreateField>
            </>
          )}
        </fieldset>

        {error && (
          <p
            id="planning-create-error"
            className="planning-editor__error"
            role="alert"
          >
            {error}
          </p>
        )}

        <footer className="planning-editor__actions">
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
            {saving
              ? copy.actions.saving
              : taskMode
                ? copy.actions.addTask
                : copy.actions.addSchedule}
          </button>
        </footer>
      </form>
    </dialog>
  );
}

function CreateField({
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

export function validateScheduleTimes(
  startsAt: string,
  endsAt: string,
): string | undefined {
  if (!startsAt || !endsAt) return copy.forms.scheduleTimeRequired;
  const start = seoulLocalDateTimeToIso(startsAt);
  const end = seoulLocalDateTimeToIso(endsAt);
  if (!start || !end || new Date(end).getTime() <= new Date(start).getTime()) {
    return copy.forms.scheduleTimeOrder;
  }
  return undefined;
}

export function defaultScheduleRange(now = new Date()) {
  const start = new Date(now);
  start.setUTCSeconds(0, 0);
  const minutes = start.getUTCMinutes();
  start.setUTCMinutes(minutes < 30 ? 30 : 60);
  const end = new Date(start.getTime() + 60 * 60 * 1_000);
  return {
    startsAt: isoToSeoulLocalDateTime(start.toISOString()),
    endsAt: isoToSeoulLocalDateTime(end.toISOString()),
  };
}

function localInputToIso(value: string): string {
  const iso = seoulLocalDateTimeToIso(value);
  if (!iso) throw new Error("invalid date time");
  return iso;
}
