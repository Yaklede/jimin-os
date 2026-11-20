import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import type { Task } from "../api/planning";
import { copy } from "../copy";
import { calendarTaskPreviewCopy as text } from "../copy/calendarTaskPreview";
import { registerMobileBackHandler } from "../mobileBack";
import "./calendar-task-preview.css";

type PreviewSource = "pointer" | "focus" | "touch";
type Preview = {
  anchor: HTMLButtonElement;
  ids: string[];
  source: PreviewSource;
  period: number;
};

const gutter = 8;
const gap = 4;
const previewLimit = 3;
const dueFormat = new Intl.DateTimeFormat("ko-KR", {
  month: "long",
  day: "numeric",
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
  const active = preview;
  const pinned = active?.source === "touch";
  const items = active
    ? active.ids
        .slice(0, previewLimit)
        .flatMap((id) => tasks?.find((task) => task.id === id) ?? [])
    : [];
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

  function open(
    anchor: HTMLButtonElement,
    source: PreviewSource,
    candidates: readonly Task[],
  ) {
    if (loading || !candidates.length) return;
    keep();
    const ids = candidates.map(({ id }) => id);
    setPreview((previous) => {
      const sameAnchor =
        previous?.anchor === anchor && previous.period === period;
      if (source === "touch" && sameAnchor && previous.source === "touch")
        return undefined;
      if (
        sameAnchor &&
        (previous.source === "touch" ||
          (previous.source === source &&
            previous.ids.length === ids.length &&
            previous.ids.every((id, index) => id === ids[index])))
      )
        return previous;
      return { anchor, source, ids, period };
    });
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

  useEffect(() => {
    if (!anchor) return;
    function onDismissKey(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      closeAndRestore();
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
      if (event.target instanceof Node && panel.current?.contains(event.target))
        return;
      close();
    }
    document.addEventListener("keydown", onDismissKey, true);
    document.addEventListener("pointerdown", onOutsidePress, true);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("keydown", onDismissKey, true);
      document.removeEventListener("pointerdown", onOutsidePress, true);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [anchor, close, closeAndRestore]);

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

  return {
    open,
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
              aria-label={pinned ? text.title : undefined}
              className="calendar-task-preview"
              data-pinned={pinned}
              onPointerEnter={keep}
              onPointerLeave={leave}
            >
              {pinned && (
                <header className="calendar-task-preview__header">
                  <strong>{text.title}</strong>
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
              {!pinned && active.ids.length > 1 && (
                <strong>{copy.home.taskGroupCount(active.ids.length)}</strong>
              )}
              <ul>
                {items.map((task) => (
                  <li key={task.id}>
                    <strong>{task.title}</strong>
                    <dl>
                      <dt>{copy.forms.assignee}</dt>
                      <dd>
                        {task.assigneeName?.trim() ||
                          copy.home.unassignedTaskGroup}
                      </dd>
                      <dt>{text.deadline}</dt>
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
                      <dt>{text.status}</dt>
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
              {active.ids.length > previewLimit && (
                <span>
                  {copy.schedule.calendarMore(active.ids.length - previewLimit)}
                </span>
              )}
            </div>,
            document.body,
          )
        : null,
  };
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
