import {
  ArrowRight,
  CalendarDays,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Clock3,
  FolderKanban,
  ListTodo,
  Pencil,
  UserRound,
} from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import { type ScheduleEntry, type Task } from "../api/planning";
import { type Project } from "../api/projects";
import {
  type AssistantPresentation,
  type AssistantPresentationSection,
} from "../assistantPresentation";
import {
  groupTaskPresentationItems,
  initialTaskGroupView,
  taskGroupItemSummary,
  type TaskGroupView,
} from "../assistantTaskGrouping";
import { copy } from "../copy";

type AssistantInteractiveCanvasProps = {
  presentation: AssistantPresentation;
  onContinue(): void;
  onCompleteTask(task: Pick<Task, "id" | "projectId">): Promise<Task>;
  onEditTask(task: Pick<Task, "id" | "projectId">): void | Promise<void>;
  onEditSchedule(
    entry: Pick<ScheduleEntry, "id" | "startsAt">,
  ): void | Promise<void>;
  onOpenTask(task: Pick<Task, "id" | "projectId">): void | Promise<void>;
  onOpenProject(
    project: Pick<Project, "id" | "workspaceId">,
  ): void | Promise<void>;
  onOpenSchedule(
    entry: Pick<ScheduleEntry, "id" | "startsAt">,
  ): void | Promise<void>;
};

export function AssistantInteractiveCanvas({
  presentation,
  onContinue,
  onCompleteTask,
  onEditTask,
  onEditSchedule,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
}: AssistantInteractiveCanvasProps) {
  const canvasRef = useRef<HTMLElement | null>(null);
  const mountedRef = useRef(true);
  const initialSection = sectionForItem(
    presentation.sections,
    presentation.focusItemId,
  );
  const [activeKind, setActiveKind] = useState(initialSection?.kind);
  const [selectedItemId, setSelectedItemId] = useState(
    presentation.focusItemId ?? initialSection?.items[0]?.id,
  );
  const [taskGroupView, setTaskGroupView] = useState<TaskGroupView>(() =>
    initialTaskGroupView(initialSection),
  );
  const [collapsedTaskGroups, setCollapsedTaskGroups] = useState<Set<string>>(
    () => new Set(),
  );
  const [opening, setOpening] = useState(false);
  const [completingTaskId, setCompletingTaskId] = useState<string>();
  const [completedTaskIds, setCompletedTaskIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [openError, setOpenError] = useState<string>();

  const activeSection =
    presentation.sections.find((section) => section.kind === activeKind) ??
    presentation.sections[0];
  const activeTaskItems =
    activeSection?.kind === "tasks"
      ? activeSection.items.filter((item) => !completedTaskIds.has(item.id))
      : [];
  const activeItems =
    activeSection?.kind === "tasks"
      ? activeTaskItems
      : (activeSection?.items ?? []);
  const selectedItem =
    activeItems.find((item) => item.id === selectedItemId) ?? activeItems[0];
  const selectedItemCanOpen = selectedItem
    ? canOpenPresentationItem(selectedItem)
    : false;
  const taskGroups =
    activeSection?.kind === "tasks"
      ? groupTaskPresentationItems(activeTaskItems, taskGroupView)
      : [];

  useEffect(() => {
    mountedRef.current = true;
    canvasRef.current?.focus({ preventScroll: true });
    return () => {
      mountedRef.current = false;
    };
  }, []);

  function selectSection(section: AssistantPresentationSection) {
    setActiveKind(section.kind);
    const focusedItem = section.items.find(
      (item) => item.id === presentation.focusItemId,
    );
    setSelectedItemId(focusedItem?.id ?? section.items[0]?.id);
    setTaskGroupView(initialTaskGroupView(section));
    setCollapsedTaskGroups(new Set());
    setOpenError(undefined);
  }

  function selectTaskGroupView(view: TaskGroupView) {
    setTaskGroupView(view);
    setCollapsedTaskGroups(new Set());
  }

  function toggleTaskGroup(groupId: string) {
    setCollapsedTaskGroups((current) => {
      const next = new Set(current);
      if (next.has(groupId)) next.delete(groupId);
      else next.add(groupId);
      return next;
    });
  }

  async function openSelectedItem() {
    if (!selectedItem || opening) return;
    setOpening(true);
    setOpenError(undefined);
    try {
      if (selectedItem.type === "task") {
        await onOpenTask(selectedItem);
      } else if (selectedItem.type === "schedule") {
        await onOpenSchedule(selectedItem);
      } else {
        await onOpenProject(selectedItem);
      }
    } catch {
      if (mountedRef.current) setOpenError(copy.home.resultOpenFailed);
    } finally {
      if (mountedRef.current) setOpening(false);
    }
  }

  async function editSelectedItem() {
    if (!selectedItem || selectedItem.type === "project" || opening) return;
    setOpening(true);
    setOpenError(undefined);
    try {
      if (selectedItem.type === "task") {
        await onEditTask(selectedItem);
      } else {
        await onEditSchedule(selectedItem);
      }
    } catch {
      if (mountedRef.current) setOpenError(copy.home.resultEditFailed);
    } finally {
      if (mountedRef.current) setOpening(false);
    }
  }

  async function completePresentationTask(
    task: Pick<Task, "id" | "projectId">,
  ) {
    if (completingTaskId) return;
    setCompletingTaskId(task.id);
    setOpenError(undefined);
    try {
      await onCompleteTask(task);
      if (!mountedRef.current) return;
      setCompletedTaskIds((current) => new Set(current).add(task.id));
    } catch {
      if (mountedRef.current) {
        setOpenError(copy.home.resultTaskCompleteFailed);
      }
    } finally {
      if (mountedRef.current) setCompletingTaskId(undefined);
    }
  }

  function moveBetweenTabs(event: KeyboardEvent<HTMLButtonElement>) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
      return;
    }
    const tabs = Array.from(
      event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
        '[role="tab"]',
      ) ?? [],
    );
    const currentIndex = tabs.indexOf(event.currentTarget);
    if (currentIndex < 0 || !tabs.length) return;
    event.preventDefault();
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? tabs.length - 1
          : (currentIndex +
              (event.key === "ArrowRight" ? 1 : -1) +
              tabs.length) %
            tabs.length;
    tabs[nextIndex]?.focus();
    tabs[nextIndex]?.click();
  }

  return (
    <section
      ref={canvasRef}
      className="assistant-canvas"
      aria-labelledby="assistant-canvas-title"
      tabIndex={-1}
    >
      <header className="assistant-canvas__header">
        <div>
          <p>{copy.home.resultEyebrow}</p>
          <h3 id="assistant-canvas-title">{presentation.title}</h3>
        </div>
      </header>
      <p className="assistant-canvas__summary" aria-live="polite">
        {presentation.summary}
      </p>

      {!presentation.sections.length ? (
        <button
          className="secondary-button assistant-canvas__follow-up focus-visible-control"
          type="button"
          onClick={onContinue}
        >
          {copy.home.continueRequest}
          <ArrowRight aria-hidden="true" />
        </button>
      ) : (
        <>
          <div
            className="assistant-canvas__tabs"
            role="tablist"
            aria-label={copy.home.resultSectionsLabel}
          >
            {presentation.sections.map((section) => {
              const selected = activeSection?.kind === section.kind;
              return (
                <button
                  key={section.kind}
                  id={`assistant-tab-${section.kind}`}
                  className="assistant-canvas__tab focus-visible-control"
                  type="button"
                  role="tab"
                  aria-selected={selected}
                  aria-controls={`assistant-panel-${section.kind}`}
                  tabIndex={selected ? 0 : -1}
                  onKeyDown={moveBetweenTabs}
                  onClick={() => selectSection(section)}
                >
                  <SectionIcon kind={section.kind} />
                  <span>{section.title}</span>
                  <small>{copy.home.resultCount(section.items.length)}</small>
                </button>
              );
            })}
          </div>

          {activeSection && (
            <>
              {activeSection.kind === "tasks" && (
                <div className="assistant-canvas__view-controls">
                  <span>{copy.home.taskGroupViewLabel}</span>
                  <div
                    role="group"
                    aria-label={copy.home.taskGroupViewLabel}
                    className="assistant-canvas__view-switch"
                  >
                    <button
                      type="button"
                      className="focus-visible-control"
                      aria-pressed={taskGroupView === "assignee"}
                      onClick={() => selectTaskGroupView("assignee")}
                    >
                      <UserRound aria-hidden="true" />
                      {copy.home.groupTasksByAssignee}
                    </button>
                    <button
                      type="button"
                      className="focus-visible-control"
                      aria-pressed={taskGroupView === "date"}
                      onClick={() => selectTaskGroupView("date")}
                    >
                      <CalendarDays aria-hidden="true" />
                      {copy.home.groupTasksByDate}
                    </button>
                  </div>
                </div>
              )}
              <div
                className="assistant-canvas__workspace"
                data-layout={presentation.layout}
                data-view={activeSection.view}
                data-grouped={activeSection.kind === "tasks"}
                id={`assistant-panel-${activeSection.kind}`}
                role="tabpanel"
                aria-labelledby={`assistant-tab-${activeSection.kind}`}
              >
                {activeSection.kind === "tasks" && taskGroups.length ? (
                  <div className="assistant-canvas__groups">
                    {taskGroups.map((group) => {
                      const collapsed = collapsedTaskGroups.has(group.id);
                      return (
                        <section
                          className="assistant-canvas__group"
                          key={group.id}
                        >
                          <button
                            type="button"
                            className="assistant-canvas__group-heading focus-visible-control"
                            aria-expanded={!collapsed}
                            onClick={() => toggleTaskGroup(group.id)}
                          >
                            {taskGroupView === "assignee" ? (
                              <UserRound aria-hidden="true" />
                            ) : (
                              <Clock3 aria-hidden="true" />
                            )}
                            <span>
                              <strong>{group.title}</strong>
                              <small>
                                {copy.home.taskGroupCount(group.items.length)}
                              </small>
                            </span>
                            <ChevronDown aria-hidden="true" />
                          </button>
                          {!collapsed && (
                            <ul className="assistant-canvas__items">
                              {group.items.map((item) => (
                                <li key={item.id}>
                                  <PresentationItemButton
                                    item={item}
                                    section={activeSection}
                                    selected={item.id === selectedItem.id}
                                    summary={taskGroupItemSummary(
                                      item,
                                      taskGroupView,
                                    )}
                                    completing={completingTaskId === item.id}
                                    onComplete={() =>
                                      void completePresentationTask(item)
                                    }
                                    onSelect={() => {
                                      setSelectedItemId(item.id);
                                      setOpenError(undefined);
                                    }}
                                  />
                                </li>
                              ))}
                            </ul>
                          )}
                        </section>
                      );
                    })}
                  </div>
                ) : activeSection.kind === "tasks" ? (
                  <p
                    className="assistant-canvas__empty"
                    role="status"
                    aria-live="polite"
                  >
                    {copy.home.resultTasksHandled}
                  </p>
                ) : (
                  <ul className="assistant-canvas__items">
                    {activeSection.items.map((item) => (
                      <li key={item.id}>
                        <PresentationItemButton
                          item={item}
                          section={activeSection}
                          selected={item.id === selectedItem.id}
                          completing={false}
                          onSelect={() => {
                            setSelectedItemId(item.id);
                            setOpenError(undefined);
                          }}
                        />
                      </li>
                    ))}
                  </ul>
                )}
                {selectedItem && (
                  <article
                    className="assistant-canvas__detail"
                    aria-label={copy.home.resultDetailsLabel}
                    aria-live="polite"
                  >
                    <ItemDetail
                      item={selectedItem}
                      opening={opening}
                      completing={completingTaskId === selectedItem.id}
                      error={openError}
                      canOpen={selectedItemCanOpen}
                      onComplete={() =>
                        selectedItem.type === "task"
                          ? void completePresentationTask(selectedItem)
                          : undefined
                      }
                      onEdit={() => void editSelectedItem()}
                      onOpen={() => void openSelectedItem()}
                    />
                  </article>
                )}
              </div>
            </>
          )}
        </>
      )}
    </section>
  );
}

function PresentationItemButton({
  item,
  section,
  selected,
  summary,
  completing,
  onComplete,
  onSelect,
}: {
  item: AssistantPresentationSection["items"][number];
  section: AssistantPresentationSection;
  selected: boolean;
  summary?: string;
  completing: boolean;
  onComplete?: () => void;
  onSelect(): void;
}) {
  return (
    <div className="assistant-canvas__item" data-selected={selected}>
      {item.type === "task" && item.status === "open" && onComplete ? (
        <button
          className="assistant-canvas__complete focus-visible-control"
          type="button"
          disabled={completing}
          aria-label={copy.home.completeTask(item.title)}
          onClick={onComplete}
        >
          {completing ? (
            <span className="button-spinner" aria-hidden="true" />
          ) : (
            <Circle aria-hidden="true" />
          )}
        </button>
      ) : (
        <ItemMarker section={section} />
      )}
      <button
        className="assistant-canvas__item-main focus-visible-control"
        type="button"
        aria-current={selected}
        onClick={onSelect}
      >
        <span>
          <strong>{item.title}</strong>
          <small>{summary ?? itemSummary(item)}</small>
        </span>
        <ChevronRight aria-hidden="true" />
      </button>
    </div>
  );
}

function sectionForItem(
  sections: AssistantPresentationSection[],
  itemId?: string,
) {
  return (
    sections.find((section) =>
      section.items.some((item) => item.id === itemId),
    ) ?? sections[0]
  );
}

function SectionIcon({ kind }: { kind: AssistantPresentationSection["kind"] }) {
  if (kind === "tasks") return <ListTodo aria-hidden="true" />;
  if (kind === "schedule") return <CalendarDays aria-hidden="true" />;
  return <FolderKanban aria-hidden="true" />;
}

function ItemMarker({ section }: { section: AssistantPresentationSection }) {
  if (section.kind === "tasks" && section.view === "checklist") {
    return <Circle className="assistant-canvas__marker" aria-hidden="true" />;
  }
  if (section.kind === "schedule" && section.view === "timeline") {
    return (
      <span className="assistant-canvas__timeline-dot" aria-hidden="true" />
    );
  }
  return <SectionIcon kind={section.kind} />;
}

function itemSummary(
  item: AssistantPresentationSection["items"][number],
): string {
  if (item.type === "task") {
    return item.status === "open"
      ? item.projectTitle || copy.home.unassignedTask
      : copy.home.taskStatus(item.status);
  }
  if (item.type === "schedule") {
    return item.status === "cancelled"
      ? copy.home.scheduleStatus(item.status)
      : `${formatTime(item.startsAt)}–${formatTime(item.endsAt)}`;
  }
  return item.status === "active"
    ? item.nextAction || item.objective || copy.projects.noNextAction
    : copy.home.projectStatus(item.status);
}

function ItemDetail({
  item,
  opening,
  completing,
  error,
  canOpen,
  onComplete,
  onEdit,
  onOpen,
}: {
  item: AssistantPresentationSection["items"][number];
  opening: boolean;
  completing: boolean;
  error: string | undefined;
  canOpen: boolean;
  onComplete(): void;
  onEdit(): void;
  onOpen(): void;
}) {
  if (item.type === "task") {
    return (
      <>
        <span className="assistant-canvas__detail-icon" aria-hidden="true">
          <CheckCircle2 />
        </span>
        <div className="assistant-canvas__detail-copy">
          <p>{`${copy.home.taskStatus(item.status)} · ${copy.home.taskPriority(item.priority)}`}</p>
          <h4>{item.title}</h4>
          <span>{item.projectTitle || copy.home.unassignedTask}</span>
          <span>
            {copy.projects.taskAssignee(item.assigneeName ?? undefined)}
          </span>
          {item.dueAt && (
            <time dateTime={item.dueAt}>{formatDate(item.dueAt)}</time>
          )}
        </div>
        <div className="assistant-canvas__detail-actions">
          {item.status === "open" && (
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={completing || opening}
              aria-busy={completing}
              onClick={onComplete}
            >
              {completing ? (
                <>
                  <span className="button-spinner" aria-hidden="true" />
                  {copy.home.resultTaskCompleting}
                </>
              ) : (
                <>
                  <CheckCircle2 aria-hidden="true" />
                  {copy.home.resultTaskComplete}
                </>
              )}
            </button>
          )}
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={opening || completing}
            aria-busy={opening}
            onClick={onEdit}
          >
            <Pencil aria-hidden="true" />
            {copy.home.editTaskAction}
          </button>
          {canOpen && (
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={opening || completing}
              aria-busy={opening}
              onClick={onOpen}
            >
              <DestinationActionContent
                opening={opening}
                label={copy.home.openTaskAction}
              />
            </button>
          )}
        </div>
        {error && <ResultOpenError message={error} />}
      </>
    );
  }
  if (item.type === "schedule") {
    return (
      <>
        <span className="assistant-canvas__detail-icon" aria-hidden="true">
          <CalendarDays />
        </span>
        <div className="assistant-canvas__detail-copy">
          <p>{`${copy.home.scheduleStatus(item.status)} · ${formatDate(item.startsAt)}`}</p>
          <h4>{item.title}</h4>
          <span>{`${formatTime(item.startsAt)}–${formatTime(item.endsAt)}`}</span>
        </div>
        <div className="assistant-canvas__detail-actions">
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={opening}
            aria-busy={opening}
            onClick={onEdit}
          >
            <Pencil aria-hidden="true" />
            {copy.home.editScheduleAction}
          </button>
          {canOpen && (
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={opening}
              aria-busy={opening}
              onClick={onOpen}
            >
              <DestinationActionContent
                opening={opening}
                label={copy.home.openScheduleAction}
              />
            </button>
          )}
        </div>
        {error && <ResultOpenError message={error} />}
      </>
    );
  }
  return (
    <>
      <span className="assistant-canvas__detail-icon" aria-hidden="true">
        <FolderKanban />
      </span>
      <div className="assistant-canvas__detail-copy">
        <p>{`${copy.home.projectStatus(item.status)} · ${copy.home.projectTaskCount(item.openTaskCount)}`}</p>
        <h4>{item.title}</h4>
        {item.status !== "removed" && (
          <span>
            {item.nextAction
              ? `${copy.home.projectNextActionLabel} · ${item.nextAction}`
              : item.objective || copy.projects.noNextAction}
          </span>
        )}
      </div>
      {canOpen && (
        <button
          className="primary-button focus-visible-control"
          type="button"
          disabled={opening}
          aria-busy={opening}
          onClick={onOpen}
        >
          <DestinationActionContent
            opening={opening}
            label={copy.home.openProjectAction}
          />
        </button>
      )}
      {error && <ResultOpenError message={error} />}
    </>
  );
}

function DestinationActionContent({
  opening,
  label,
}: {
  opening: boolean;
  label: string;
}) {
  return opening ? (
    <>
      <span className="button-spinner" aria-hidden="true" />
      {copy.home.resultOpening}
    </>
  ) : (
    <>
      {label}
      <ArrowRight aria-hidden="true" />
    </>
  );
}

function ResultOpenError({ message }: { message: string }) {
  return (
    <p className="assistant-canvas__open-error" role="alert">
      {message}
    </p>
  );
}

export function canOpenPresentationItem(
  item: AssistantPresentationSection["items"][number],
  now = new Date(),
): boolean {
  if (item.type === "project") return item.status !== "removed";
  if (item.type === "schedule") return item.status !== "cancelled";
  if (item.status !== "open") return false;
  if (item.projectId) return true;
  if (!item.dueAt) return true;
  const endOfToday = new Date(now);
  endOfToday.setHours(24, 0, 0, 0);
  return new Date(item.dueAt).getTime() < endOfToday.getTime();
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    weekday: "short",
  }).format(new Date(value));
}
