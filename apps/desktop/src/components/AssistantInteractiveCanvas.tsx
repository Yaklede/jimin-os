import {
  ArrowLeft,
  ArrowRight,
  CalendarDays,
  CheckCircle2,
  ChevronRight,
  Circle,
  FolderKanban,
  ListTodo,
} from "lucide-react";
import { useState, type KeyboardEvent } from "react";

import { type ScheduleEntry, type Task } from "../api/planning";
import { type Project } from "../api/projects";
import {
  type AssistantPresentation,
  type AssistantPresentationSection,
} from "../assistantPresentation";
import { copy } from "../copy";

type AssistantInteractiveCanvasProps = {
  presentation: AssistantPresentation;
  onOpenAssistant(): void;
  onOpenTask(task: Pick<Task, "id" | "projectId">): void;
  onOpenProject(project: Pick<Project, "id" | "workspaceId">): void;
  onOpenSchedule(entry: Pick<ScheduleEntry, "id">): void;
  onReset(): void;
};

export function AssistantInteractiveCanvas({
  presentation,
  onOpenAssistant,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
  onReset,
}: AssistantInteractiveCanvasProps) {
  const initialSection = sectionForItem(
    presentation.sections,
    presentation.focusItemId,
  );
  const [activeKind, setActiveKind] = useState(initialSection?.kind);
  const [selectedItemId, setSelectedItemId] = useState(
    presentation.focusItemId ?? initialSection?.items[0]?.id,
  );

  const activeSection =
    presentation.sections.find((section) => section.kind === activeKind) ??
    presentation.sections[0];
  const selectedItem =
    activeSection?.items.find((item) => item.id === selectedItemId) ??
    activeSection?.items[0];

  function selectSection(section: AssistantPresentationSection) {
    setActiveKind(section.kind);
    const focusedItem = section.items.find(
      (item) => item.id === presentation.focusItemId,
    );
    setSelectedItemId(focusedItem?.id ?? section.items[0]?.id);
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
      className="assistant-canvas"
      aria-labelledby="assistant-canvas-title"
    >
      <header className="assistant-canvas__header">
        <div>
          <p>{copy.home.resultEyebrow}</p>
          <h3 id="assistant-canvas-title">{presentation.title}</h3>
        </div>
        <button
          className="text-button focus-visible-control"
          type="button"
          onClick={onReset}
        >
          <ArrowLeft aria-hidden="true" />
          {copy.home.backToBriefing}
        </button>
      </header>
      <p className="assistant-canvas__summary" aria-live="polite">
        {presentation.summary}
      </p>

      {!presentation.sections.length ? (
        <button
          className="secondary-button assistant-canvas__follow-up focus-visible-control"
          type="button"
          onClick={onOpenAssistant}
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

          {activeSection && selectedItem && (
            <div
              className="assistant-canvas__workspace"
              data-layout={presentation.layout}
              data-view={activeSection.view}
              id={`assistant-panel-${activeSection.kind}`}
              role="tabpanel"
              aria-labelledby={`assistant-tab-${activeSection.kind}`}
            >
              <ul className="assistant-canvas__items">
                {activeSection.items.map((item) => (
                  <li key={item.id}>
                    <button
                      className="assistant-canvas__item focus-visible-control"
                      type="button"
                      data-selected={item.id === selectedItem.id}
                      aria-current={item.id === selectedItem.id}
                      onClick={() => setSelectedItemId(item.id)}
                    >
                      <ItemMarker section={activeSection} />
                      <span>
                        <strong>{item.title}</strong>
                        <small>{itemSummary(item)}</small>
                      </span>
                      <ChevronRight aria-hidden="true" />
                    </button>
                  </li>
                ))}
              </ul>
              <article
                className="assistant-canvas__detail"
                aria-label={copy.home.resultDetailsLabel}
                aria-live="polite"
              >
                <ItemDetail
                  item={selectedItem}
                  onOpenTask={onOpenTask}
                  onOpenProject={onOpenProject}
                  onOpenSchedule={onOpenSchedule}
                />
              </article>
            </div>
          )}
        </>
      )}
    </section>
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
    return item.projectTitle || copy.home.unassignedTask;
  }
  if (item.type === "schedule") {
    return `${formatTime(item.startsAt)}–${formatTime(item.endsAt)}`;
  }
  return item.nextAction || item.objective || copy.projects.noNextAction;
}

function ItemDetail({
  item,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
}: {
  item: AssistantPresentationSection["items"][number];
  onOpenTask(task: Pick<Task, "id" | "projectId">): void;
  onOpenProject(project: Pick<Project, "id" | "workspaceId">): void;
  onOpenSchedule(entry: Pick<ScheduleEntry, "id">): void;
}) {
  if (item.type === "task") {
    return (
      <>
        <span className="assistant-canvas__detail-icon" aria-hidden="true">
          <CheckCircle2 />
        </span>
        <div className="assistant-canvas__detail-copy">
          <p>{copy.home.taskPriority(item.priority)}</p>
          <h4>{item.title}</h4>
          <span>{item.projectTitle || copy.home.unassignedTask}</span>
          {item.dueAt && (
            <time dateTime={item.dueAt}>{formatDate(item.dueAt)}</time>
          )}
        </div>
        <button
          className="primary-button focus-visible-control"
          type="button"
          onClick={() => onOpenTask(item)}
        >
          {copy.home.openTaskAction}
          <ArrowRight aria-hidden="true" />
        </button>
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
          <p>{formatDate(item.startsAt)}</p>
          <h4>{item.title}</h4>
          <span>{`${formatTime(item.startsAt)}–${formatTime(item.endsAt)}`}</span>
        </div>
        <button
          className="primary-button focus-visible-control"
          type="button"
          onClick={() => onOpenSchedule(item)}
        >
          {copy.home.openScheduleAction}
          <ArrowRight aria-hidden="true" />
        </button>
      </>
    );
  }
  return (
    <>
      <span className="assistant-canvas__detail-icon" aria-hidden="true">
        <FolderKanban />
      </span>
      <div className="assistant-canvas__detail-copy">
        <p>{copy.home.projectTaskCount(item.openTaskCount)}</p>
        <h4>{item.title}</h4>
        <span>
          {item.nextAction
            ? `${copy.home.projectNextActionLabel} · ${item.nextAction}`
            : item.objective || copy.projects.noNextAction}
        </span>
      </div>
      <button
        className="primary-button focus-visible-control"
        type="button"
        onClick={() => onOpenProject(item)}
      >
        {copy.home.openProjectAction}
        <ArrowRight aria-hidden="true" />
      </button>
    </>
  );
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
