import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import type { Task } from "../api/planning";
import { copy } from "../copy";
import { calendarTaskPreviewCopy as text } from "../copy/calendarTaskPreview";
import { registerMobileBackHandler } from "../mobileBack";
import { samePlanningDay } from "../planningRange";
import "./calendar-task-preview.css";

type PreviewSource = "pointer" | "focus" | "touch";
type Preview = {
  anchor: HTMLButtonElement;
  date: Date;
  source: PreviewSource;
  period: number;
};

const gutter = 8;
const gap = 4;
const previewDateFormat = new Intl.DateTimeFormat("ko-KR", {
  month: "long",
  day: "numeric",
  weekday: "short",
});
const dueFormat = new Intl.DateTimeFormat("ko-KR", {
  hour: "numeric",
  minute: "2-digit",
});

export function useCalendarTaskPreview(
  tasks: readonly Task[] | undefined,
  period: number,
  loading: boolean,
  onEditTask?: (task: Task) => void,
) {
  const id = useId();
  const [preview, setPreview] = useState<Preview>();
  if (
    preview &&
    (preview.period !== period || (loading && preview.source !== "touch"))
  ) {
    setPreview(undefined);
  }
  const dismissTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const panel = useRef<HTMLDivElement>(null);
  const pointerPosition = useRef<string>(undefined);
  const dismissedAt = useRef<string>(undefined);
  const active = preview;
  const pinned = active?.source === "touch";
  const items = active ? calendarTasksForDay(tasks, active.date) : [];
  const visible = items.length > 0;
  const anchor = visible ? active?.anchor : undefined;

  const keep = useCallback(() => {
    clearTimeout(dismissTimer.current);
  }, []);

  const close = useCallback(() => {
    keep();
    setPreview(undefined);
  }, [keep]);

  const closeAndRestore = useCallback(() => {
    close();
    if (pinned && anchor?.isConnected) anchor.focus({ preventScroll: true });
  }, [anchor, close, pinned]);

  function open(anchor: HTMLButtonElement, source: PreviewSource, date: Date) {
    if (loading) return;
    keep();
    setPreview((previous) => {
      const sameAnchor =
        previous?.anchor === anchor &&
        previous.period === period &&
        samePlanningDay(previous.date, date);
      if (source === "touch" && sameAnchor && previous.source === "touch")
        return undefined;
      if (
        sameAnchor &&
        (previous.source === "touch" || previous.source === source)
      )
        return previous;
      return { anchor, source, date, period };
    });
  }

  function openFromPointer(
    event: ReactPointerEvent<HTMLButtonElement>,
    date: Date,
  ) {
    if (event.pointerType !== "mouse") return;
    const position = `${event.clientX}:${event.clientY}`;
    // Removing a tooltip can expose a trigger under a stationary pointer.
    // Escape should keep it closed until the pointer actually moves.
    if (dismissedAt.current === position) return;
    dismissedAt.current = undefined;
    pointerPosition.current = position;
    open(event.currentTarget, "pointer", date);
  }

  function leave() {
    if (active?.source !== "pointer") return;
    keep();
    // Allow the pointer to cross the gap into the preview itself.
    dismissTimer.current = setTimeout(close, 180);
  }

  function blur(trigger: HTMLButtonElement) {
    if (active?.source === "focus" && active.anchor === trigger) close();
  }

  useEffect(() => () => clearTimeout(dismissTimer.current), []);

  useLayoutEffect(() => {
    if (!anchor) return;
    function onDismissKey(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      dismissedAt.current = pointerPosition.current;
      closeAndRestore();
    }
    function trackPointer(event: PointerEvent) {
      if (event.pointerType === "mouse")
        pointerPosition.current = `${event.clientX}:${event.clientY}`;
    }
    function onOutsidePress(event: PointerEvent) {
      if (
        event.target instanceof Node &&
        !anchor?.contains(event.target) &&
        !panel.current?.contains(event.target)
      )
        close();
    }
    function onScroll(event: Event) {
      // Selecting a date can reposition the calendar on Android. A fixed
      // touch preview stays open without depending on its trigger's position.
      if (pinned) return;
      if (event.target instanceof Node && panel.current?.contains(event.target))
        return;
      close();
    }
    document.addEventListener("keydown", onDismissKey, true);
    document.addEventListener("pointerdown", onOutsidePress, true);
    document.addEventListener("pointermove", trackPointer, true);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("keydown", onDismissKey, true);
      document.removeEventListener("pointerdown", onOutsidePress, true);
      document.removeEventListener("pointermove", trackPointer, true);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [anchor, close, closeAndRestore, pinned]);

  useLayoutEffect(() => {
    if (!anchor || !pinned) return;
    panel.current
      ?.querySelector<HTMLButtonElement>("button")
      ?.focus({ preventScroll: true });
    return registerMobileBackHandler(() => {
      closeAndRestore();
      return true;
    }, 115);
  }, [anchor, pinned, closeAndRestore]);

  const itemVersions = items
    .map((item) => `${item.id}:${item.version}`)
    .join(",");
  useLayoutEffect(() => {
    const element = panel.current;
    if (!anchor || !element || pinned) return;
    const place = () => {
      const rect = anchor.getBoundingClientRect();
      const { clientWidth, clientHeight } = document.documentElement;
      const height = element.scrollHeight;
      const below = clientHeight - rect.bottom - gap - gutter;
      const above = rect.top - gap - gutter;
      const placeBelow = height <= below || below >= above;
      const maxHeight = Math.max(0, placeBelow ? below : above);
      element.style.maxHeight = `${maxHeight}px`;
      element.style.top = `${Math.max(gutter, placeBelow ? rect.bottom + gap : rect.top - gap - Math.min(height, maxHeight))}px`;
      element.style.left = `${Math.max(gutter, Math.min(rect.left, clientWidth - element.offsetWidth - gutter))}px`;
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [anchor, itemVersions, pinned]);

  const heading = active
    ? text.title(previewDateFormat.format(active.date), items.length)
    : undefined;

  return {
    open,
    openFromPointer,
    leave,
    blur,
    close,
    descriptionId: anchor && !pinned ? id : undefined,
    anchor,
    pinned,
    content:
      anchor && active
        ? createPortal(
            <div
              ref={panel}
              id={id}
              role={pinned ? "dialog" : "tooltip"}
              aria-label={pinned ? heading : undefined}
              className="calendar-task-preview"
              data-pinned={pinned}
              onPointerEnter={keep}
              onPointerLeave={leave}
            >
              {pinned && (
                <header className="calendar-task-preview__header">
                  <strong>{heading}</strong>
                  <button
                    className="icon-button focus-visible-control"
                    type="button"
                    aria-label={text.close}
                    onClick={closeAndRestore}
                  >
                    <X aria-hidden="true" />
                  </button>
                </header>
              )}
              {!pinned && <strong>{heading}</strong>}
              <ul>
                {items.map((task) => (
                  <li key={task.id}>
                    <strong>{task.title}</strong>
                    <dl>
                      <dt className="sr-only">{copy.forms.assignee}</dt>
                      <dd>
                        {task.assigneeName?.trim() ||
                          copy.home.unassignedTaskGroup}
                      </dd>
                      <dt className="sr-only">{text.deadline}</dt>
                      <dd>
                        {task.dueAt &&
                        Number.isFinite(Date.parse(task.dueAt)) ? (
                          <time dateTime={task.dueAt}>
                            {dueFormat.format(new Date(task.dueAt))}
                          </time>
                        ) : (
                          copy.home.noDueDateTaskGroup
                        )}
                      </dd>
                      <dt className="sr-only">{text.status}</dt>
                      <dd>{calendarTaskStatus(task)}</dd>
                    </dl>
                    {pinned && onEditTask && (
                      <button
                        className="secondary-button focus-visible-control calendar-task-preview__edit"
                        type="button"
                        aria-label={copy.home.editTask(task.title)}
                        onClick={() => {
                          closeAndRestore();
                          onEditTask(task);
                        }}
                      >
                        {text.edit}
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </div>,
            document.body,
          )
        : null,
  };
}

export function calendarTasksForDay(
  tasks: readonly Task[] | undefined,
  date: Date,
): Task[] {
  return (tasks ?? []).filter(
    (task) =>
      task.status === "open" &&
      task.dueAt &&
      samePlanningDay(new Date(task.dueAt), date),
  );
}

export function shouldPinCalendarPreview(
  event: MouseEvent,
  touched = false,
): boolean {
  const pointerType = (event as PointerEvent).pointerType;
  return (
    pointerType === "touch" ||
    (event.detail > 0 && touched) ||
    (event.detail === 0 &&
      window.matchMedia("(hover: none) and (pointer: coarse)").matches)
  );
}

export function calendarTaskStatus(
  task: Pick<Task, "status" | "workflowState">,
): string {
  if (task.status === "open") {
    if (task.workflowState === "in_progress") return text.inProgress;
    if (task.workflowState === "waiting") return text.waiting;
  }
  return copy.home.taskStatus(task.status);
}
