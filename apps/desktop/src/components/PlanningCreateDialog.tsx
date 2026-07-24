import { CalendarPlus, ListTodo, X } from "lucide-react";
import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";

import { copy } from "../copy";
import { registerMobileBackHandler } from "../mobileBack";

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
};

type PlanningCreateDialogProps = {
  kind: PlanningCreateKind | undefined;
  onClose(): void;
  onCreateTask(input: PlanningTaskCreateInput): Promise<void>;
  onCreateSchedule(input: PlanningScheduleCreateInput): Promise<void>;
};

export function PlanningCreateDialog({
  kind,
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

    setSaving(true);
    setError(undefined);
    try {
      if (taskMode) {
        await onCreateTask({
          title: nextTitle,
          notes: notes.trim() || undefined,
          priority,
          dueAt: dueAt ? localInputToIso(dueAt) : undefined,
        });
      } else {
        const scheduleError = validateScheduleTimes(startsAt, endsAt);
        if (scheduleError) {
          setError(scheduleError);
          setSaving(false);
          return;
        }
        await onCreateSchedule({
          title: nextTitle,
          notes: notes.trim() || undefined,
          startsAt: localInputToIso(startsAt),
          endsAt: localInputToIso(endsAt),
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
              <CreateField
                label={copy.forms.dueAt}
                htmlFor="planning-create-due-at"
                description={copy.forms.dueAtDescription}
              >
                <input
                  id="planning-create-due-at"
                  type="datetime-local"
                  value={dueAt}
                  onInput={(event) => setDueAt(event.currentTarget.value)}
                />
              </CreateField>
            </div>
          ) : (
            <div className="planning-editor__field-grid">
              <CreateField
                label={copy.forms.startsAt}
                htmlFor="planning-create-starts-at"
              >
                <input
                  id="planning-create-starts-at"
                  type="datetime-local"
                  required
                  value={startsAt}
                  aria-describedby={error ? "planning-create-error" : undefined}
                  onInput={(event) => {
                    setStartsAt(event.currentTarget.value);
                    setError(undefined);
                  }}
                />
              </CreateField>
              <CreateField
                label={copy.forms.endsAt}
                htmlFor="planning-create-ends-at"
              >
                <input
                  id="planning-create-ends-at"
                  type="datetime-local"
                  required
                  value={endsAt}
                  aria-describedby={error ? "planning-create-error" : undefined}
                  onInput={(event) => {
                    setEndsAt(event.currentTarget.value);
                    setError(undefined);
                  }}
                />
              </CreateField>
            </div>
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
      {description && <p>{description}</p>}
    </div>
  );
}

export function validateScheduleTimes(
  startsAt: string,
  endsAt: string,
): string | undefined {
  if (!startsAt || !endsAt) return copy.forms.scheduleTimeRequired;
  const start = new Date(startsAt);
  const end = new Date(endsAt);
  if (
    Number.isNaN(start.getTime()) ||
    Number.isNaN(end.getTime()) ||
    end <= start
  ) {
    return copy.forms.scheduleTimeOrder;
  }
  return undefined;
}

export function defaultScheduleRange(now = new Date()) {
  const start = new Date(now);
  start.setSeconds(0, 0);
  const minutes = start.getMinutes();
  start.setMinutes(minutes < 30 ? 30 : 60);
  const end = new Date(start.getTime() + 60 * 60 * 1_000);
  return {
    startsAt: dateToLocalInput(start),
    endsAt: dateToLocalInput(end),
  };
}

function dateToLocalInput(date: Date): string {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
}

function localInputToIso(value: string): string {
  return new Date(value).toISOString();
}
