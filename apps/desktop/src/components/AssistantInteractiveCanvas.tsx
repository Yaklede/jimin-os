import { TaskWorkKindBadge } from "./TaskWorkKind";
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
  RotateCcw,
  UserRound,
} from "lucide-react";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

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
import { LinkifiedText } from "./ExternalTextLink";
import { TaskSelectionControl } from "./TaskSelectionControl";

const assigneeAvatars: Record<string, string> = {
  김경주: "/images/assignee-kim-gyeongju.png",
  송천안: "/images/assignee-song-cheonan.png",
};

type AssistantInteractiveCanvasProps = {
  presentation: AssistantPresentation;
  onContinue(): void;
  onLoadTask(task: Pick<Task, "id" | "projectId">): Promise<Task>;
  onCompleteTask(task: Pick<Task, "id" | "projectId">): Promise<Task>;
  onRestoreTask(task: Pick<Task, "id" | "projectId">): Promise<Task>;
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
  onLoadTask,
  onCompleteTask,
  onRestoreTask,
  onEditTask,
  onEditSchedule,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
}: AssistantInteractiveCanvasProps) {
  const canvasRef = useRef<HTMLElement | null>(null);
  const mountedRef = useRef(true);
  const detailId = useId();
  const [mobile, setMobile] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia("(max-width: 720px)").matches,
  );
  const [expandedMobileItemId, setExpandedMobileItemId] = useState<string>();

  useEffect(() => {
    const media = window.matchMedia("(max-width: 720px)");
    const update = () => setMobile(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

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
  const [restoringTaskId, setRestoringTaskId] = useState<string>();
  const [selectedTaskDetail, setSelectedTaskDetail] = useState<Task>();
  const [taskDetailLoading, setTaskDetailLoading] = useState(false);
  const [taskDetailError, setTaskDetailError] = useState<string>();
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
  const selectedTaskStatus =
    selectedItem?.type === "task" && selectedTaskDetail?.id === selectedItem.id
      ? selectedTaskDetail.status
      : selectedItem?.type === "task"
        ? selectedItem.status
        : undefined;
  const selectedItemCanOpen = selectedItem
    ? canOpenPresentationItem(selectedItem, new Date(), selectedTaskStatus)
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

  useEffect(() => {
    if (!selectedItem || selectedItem.type !== "task") {
      setSelectedTaskDetail(undefined);
      setTaskDetailLoading(false);
      setTaskDetailError(undefined);
      return;
    }
    let active = true;
    setSelectedTaskDetail(undefined);
    setTaskDetailLoading(true);
    setTaskDetailError(undefined);
    void onLoadTask(selectedItem)
      .then((task) => {
        if (active && mountedRef.current) setSelectedTaskDetail(task);
      })
      .catch(() => {
        if (active && mountedRef.current) {
          setTaskDetailError(copy.home.resultTaskDetailsFailed);
        }
      })
      .finally(() => {
        if (active && mountedRef.current) setTaskDetailLoading(false);
      });
    return () => {
      active = false;
    };
  }, [selectedItem?.id, selectedItem?.type]);

  function selectSection(section: AssistantPresentationSection) {
    setActiveKind(section.kind);
    setExpandedMobileItemId(undefined);
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
    setExpandedMobileItemId(undefined);
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
      const completed = await onCompleteTask(task);
      if (!mountedRef.current || completed.status !== "completed") return;
      setSelectedTaskDetail(completed);
      setCompletedTaskIds((current) => new Set(current).add(task.id));
    } catch {
      if (mountedRef.current) {
        setOpenError(copy.home.resultTaskCompleteFailed);
      }
    } finally {
      if (mountedRef.current) setCompletingTaskId(undefined);
    }
  }

  async function restorePresentationTask(task: Pick<Task, "id" | "projectId">) {
    if (restoringTaskId) return;
    setRestoringTaskId(task.id);
    setOpenError(undefined);
    try {
      const restored = await onRestoreTask(task);
      if (!mountedRef.current) return;
      setSelectedTaskDetail(restored);
    } catch {
      if (mountedRef.current) {
        setOpenError(copy.home.resultTaskRestoreFailed);
      }
    } finally {
      if (mountedRef.current) setRestoringTaskId(undefined);
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

  const inlineTaskDetails = mobile && activeSection?.kind === "tasks";
  const selectedDetail = selectedItem && (
    <article
      className="assistant-canvas__detail"
      data-item-type={selectedItem.type}
      id={detailId}
      aria-label={copy.home.resultDetailsLabel}
      aria-live="polite"
    >
      <ItemDetail
        item={selectedItem}
        taskDetail={
          selectedTaskDetail?.id === selectedItem.id
            ? selectedTaskDetail
            : undefined
        }
        taskDetailLoading={taskDetailLoading}
        taskDetailError={taskDetailError}
        opening={opening}
        completing={completingTaskId === selectedItem.id}
        restoring={restoringTaskId === selectedItem.id}
        error={openError}
        canOpen={selectedItemCanOpen}
        onComplete={() =>
          selectedItem.type === "task"
            ? void completePresentationTask(selectedItem)
            : undefined
        }
        onRestore={() =>
          selectedItem.type === "task"
            ? void restorePresentationTask(selectedItem)
            : undefined
        }
        onEdit={() => void editSelectedItem()}
        onOpen={() => void openSelectedItem()}
      />
    </article>
  );

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
        <LinkifiedText text={presentation.summary} />
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
                  {section.kind !== "tasks" && (
                    <SectionIcon kind={section.kind} />
                  )}
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
                data-detail-placement={inlineTaskDetails ? "inline" : "panel"}
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
                              assigneeAvatars[group.title] ? (
                                <div
                                  className={`assistant-canvas__avatar${group.title === "송천안" ? " assistant-canvas__avatar--city" : ""}`}
                                  aria-hidden="true"
                                >
                                  <img
                                    src={assigneeAvatars[group.title]}
                                    alt=""
                                  />
                                </div>
                              ) : (
                                <UserRound aria-hidden="true" />
                              )
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
                                    selected={
                                      inlineTaskDetails
                                        ? expandedMobileItemId === item.id
                                        : item.id === selectedItem.id
                                    }
                                    expanded={
                                      inlineTaskDetails
                                        ? expandedMobileItemId === item.id
                                        : undefined
                                    }
                                    detailId={
                                      inlineTaskDetails &&
                                      expandedMobileItemId === item.id
                                        ? detailId
                                        : undefined
                                    }
                                    summary={taskGroupItemSummary(
                                      item,
                                      taskGroupView,
                                    )}
                                    status={
                                      selectedTaskDetail?.id === item.id
                                        ? selectedTaskDetail.status
                                        : item.status
                                    }
                                    completing={completingTaskId === item.id}
                                    onComplete={() =>
                                      void completePresentationTask(item)
                                    }
                                    onSelect={() => {
                                      setSelectedItemId(item.id);
                                      setOpenError(undefined);
                                      if (inlineTaskDetails) {
                                        setExpandedMobileItemId((current) =>
                                          current === item.id
                                            ? undefined
                                            : item.id,
                                        );
                                      }
                                    }}
                                  />
                                  {inlineTaskDetails &&
                                    expandedMobileItemId === item.id &&
                                    item.id === selectedItem.id &&
                                    selectedDetail}
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
                {!inlineTaskDetails && selectedDetail}
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
  expanded,
  detailId,
  summary,
  status,
  completing,
  onComplete,
  onSelect,
}: {
  item: AssistantPresentationSection["items"][number];
  section: AssistantPresentationSection;
  selected: boolean;
  expanded?: boolean;
  detailId?: string;
  summary?: string;
  status?: Task["status"];
  completing: boolean;
  onComplete?: () => void;
  onSelect(): void;
}) {
  return (
    <div className="assistant-canvas__item" data-selected={selected}>
      {item.type === "task" &&
        (status ?? item.status) === "open" &&
        onComplete && (
          <TaskSelectionControl
            title={item.title}
            className="assistant-canvas__complete focus-visible-control"
            disabled={completing}
            busy={completing}
            onComplete={onComplete}
          />
        )}
      <button
        className="assistant-canvas__item-main focus-visible-control"
        type="button"
        aria-current={selected}
        aria-expanded={expanded}
        aria-controls={detailId}
        onClick={onSelect}
      >
        <span>
          <strong>{item.title}</strong>
          <small>{summary ?? itemSummary(item)}</small>
        </span>
        {expanded === undefined ? (
          <ChevronRight aria-hidden="true" />
        ) : (
          <ChevronDown aria-hidden="true" />
        )}
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

function ItemMarker({
  section,
  taskStatus,
}: {
  section: AssistantPresentationSection;
  taskStatus?: Task["status"];
}) {
  if (section.kind === "tasks" && section.view === "checklist") {
    if (taskStatus === "completed") {
      return (
        <CheckCircle2 className="assistant-canvas__marker" aria-hidden="true" />
      );
    }
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
  taskDetail,
  taskDetailLoading,
  taskDetailError,
  opening,
  completing,
  restoring,
  error,
  canOpen,
  onComplete,
  onRestore,
  onEdit,
  onOpen,
}: {
  item: AssistantPresentationSection["items"][number];
  taskDetail: Task | undefined;
  taskDetailLoading: boolean;
  taskDetailError: string | undefined;
  opening: boolean;
  completing: boolean;
  restoring: boolean;
  error: string | undefined;
  canOpen: boolean;
  onComplete(): void;
  onRestore(): void;
  onEdit(): void;
  onOpen(): void;
}) {
  if (item.type === "task") {
    const status = taskDetail?.status ?? item.status;
    return (
      <>
        <div
          className="assistant-canvas__detail-copy"
          aria-busy={taskDetailLoading}
        >
          <div className="assistant-canvas__task-badges">
            <TaskWorkKindBadge kind={taskDetail?.workKind} />
            <span
              className="assistant-canvas__task-status"
              data-status={status}
            >
              {copy.home.taskStatus(status)}
            </span>
            <span className="assistant-canvas__task-priority">
              {copy.home.taskPriority(taskDetail?.priority ?? item.priority)}
            </span>
          </div>
          <h4 title={taskDetail?.title ?? item.title}>
            {taskDetail?.title ?? item.title}
          </h4>
          <span>{item.projectTitle || copy.home.unassignedTask}</span>
          <span>
            {copy.projects.taskAssignee(
              taskDetail?.assigneeName ?? item.assigneeName ?? undefined,
            )}
          </span>
          {(taskDetail?.dueAt ?? item.dueAt) && (
            <time dateTime={(taskDetail?.dueAt ?? item.dueAt)!}>
              {formatDate((taskDetail?.dueAt ?? item.dueAt)!)}
            </time>
          )}
          {taskDetailLoading && (
            <span className="assistant-canvas__detail-loading" role="status">
              <span className="button-spinner" aria-hidden="true" />
              {copy.home.resultTaskDetailsLoading}
            </span>
          )}
          {taskDetail?.notes && (
            <div className="assistant-canvas__task-notes">
              <strong>{copy.home.resultTaskNotesLabel}</strong>
              <p>
                <LinkifiedText text={taskDetail.notes} />
              </p>
            </div>
          )}
          {taskDetail?.completionNote && (
            <div className="assistant-canvas__task-notes">
              <strong>확인 결과</strong>
              <p>
                <LinkifiedText text={taskDetail.completionNote} />
              </p>
            </div>
          )}
          {taskDetailError && (
            <span className="assistant-canvas__detail-load-error" role="alert">
              {taskDetailError}
            </span>
          )}
        </div>
        <div className="assistant-canvas__detail-actions assistant-canvas__detail-actions--task">
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={opening || completing || restoring}
            aria-busy={opening}
            onClick={onEdit}
          >
            {copy.home.editTaskAction}
          </button>
          {status === "open" ? (
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={completing || restoring || opening}
              aria-busy={completing}
              onClick={onComplete}
            >
              {completing ? (
                <>
                  <span className="button-spinner" aria-hidden="true" />
                  {copy.home.resultTaskCompleting}
                </>
              ) : (
                <>{copy.home.resultTaskComplete}</>
              )}
            </button>
          ) : (
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={restoring || completing || opening}
              aria-busy={restoring}
              onClick={onRestore}
            >
              {restoring ? (
                <>
                  <span className="button-spinner" aria-hidden="true" />
                  {copy.home.resultTaskRestoring}
                </>
              ) : (
                <>{copy.home.resultTaskRestore}</>
              )}
            </button>
          )}
          {canOpen && (
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={opening || completing || restoring}
              aria-busy={opening}
              onClick={onOpen}
            >
              {opening ? copy.home.resultOpening : copy.home.openTaskAction}
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
  taskStatus = item.type === "task" ? item.status : undefined,
): boolean {
  if (item.type === "project") return item.status !== "removed";
  if (item.type === "schedule") return item.status !== "cancelled";
  if (taskStatus !== "open") return false;
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
