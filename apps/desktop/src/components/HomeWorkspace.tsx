import {
  ArrowUpRight,
  CalendarDays,
  CheckCircle2,
  Circle,
  ClipboardList,
  MessageCircleMore,
  Sparkles,
} from "lucide-react";
import { type ReactNode, useMemo, useState } from "react";

import { type HomeSnapshot } from "../api/home";
import { type Conversation } from "../api/agent";
import { type Task } from "../api/planning";
import { copy } from "../copy";

type HomeWorkspaceProps = {
  snapshot: HomeSnapshot | undefined;
  loading: boolean;
  error: string | undefined;
  assistantReady: boolean;
  conversations: Conversation[];
  onOpenAssistant(): void;
  onCompleteTask(task: Task): Promise<void>;
};

export function HomeWorkspace({
  snapshot,
  loading,
  error,
  assistantReady,
  conversations,
  onOpenAssistant,
  onCompleteTask,
}: HomeWorkspaceProps) {
  const [completingTaskId, setCompletingTaskId] = useState<
    string | undefined
  >();
  const greeting = useMemo(() => greetingForHour(new Date().getHours()), []);
  const nextSchedule = snapshot?.schedule[0];
  const scheduleCount = snapshot?.schedule.length ?? 0;
  const taskCount = snapshot?.tasks.length ?? 0;

  async function complete(task: Task) {
    if (completingTaskId) return;
    setCompletingTaskId(task.id);
    try {
      await onCompleteTask(task);
    } finally {
      setCompletingTaskId(undefined);
    }
  }

  return (
    <section className="home-page" aria-busy={loading}>
      <header className="home-greeting">
        <p>{greeting}</p>
        <h1>{copy.home.title}</h1>
        <span>{copy.home.description}</span>
      </header>

      {error && (
        <p className="home-inline-alert" role="alert">
          {error}
        </p>
      )}

      <section className="home-briefing" aria-labelledby="daily-briefing-title">
        <div>
          <span className="home-briefing__icon" aria-hidden="true">
            <Sparkles />
          </span>
          <div>
            <p>{copy.home.briefingLabel}</p>
            <h2 id="daily-briefing-title">
              {briefingTitle(loading, nextSchedule, scheduleCount)}
            </h2>
            <span>{briefingDescription(loading, nextSchedule, taskCount)}</span>
          </div>
        </div>
        <button
          className="home-briefing__action focus-visible-control"
          type="button"
          onClick={onOpenAssistant}
        >
          <MessageCircleMore aria-hidden="true" />
          {assistantReady ? copy.home.askAssistant : copy.home.connectAssistant}
        </button>
      </section>

      <div className="home-layout">
        <section
          className="home-panel home-panel--schedule"
          aria-labelledby="today-schedule-title"
        >
          <PanelHeading
            icon={<CalendarDays aria-hidden="true" />}
            title={copy.home.scheduleTitle}
            meta={
              loading
                ? copy.home.loadingShort
                : copy.home.scheduleCount(scheduleCount)
            }
          />
          {loading ? (
            <LoadingRows rows={3} />
          ) : snapshot?.schedule.length ? (
            <ol className="home-timeline">
              {snapshot.schedule.map((entry) => (
                <li key={entry.id}>
                  <time dateTime={entry.startsAt}>
                    {formatTime(entry.startsAt)}
                  </time>
                  <span aria-hidden="true" />
                  <div>
                    <strong>{entry.title}</strong>
                    <p>
                      {scheduleDetail(
                        entry.startsAt,
                        entry.endsAt,
                        entry.notes,
                      )}
                    </p>
                  </div>
                </li>
              ))}
            </ol>
          ) : (
            <EmptyPanel
              title={copy.home.scheduleEmptyTitle}
              description={copy.home.scheduleEmptyDescription}
            />
          )}
        </section>

        <section
          className="home-panel home-panel--tasks"
          aria-labelledby="today-task-title"
        >
          <PanelHeading
            icon={<ClipboardList aria-hidden="true" />}
            title={copy.home.taskTitle}
            meta={
              loading ? copy.home.loadingShort : copy.home.taskCount(taskCount)
            }
          />
          {loading ? (
            <LoadingRows rows={4} />
          ) : snapshot?.tasks.length ? (
            <ul className="home-task-list">
              {snapshot.tasks.map((task) => (
                <li key={task.id}>
                  <button
                    className="home-task-list__complete focus-visible-control"
                    type="button"
                    onClick={() => void complete(task)}
                    disabled={Boolean(completingTaskId)}
                    aria-label={copy.home.completeTask(task.title)}
                  >
                    {completingTaskId === task.id ? (
                      <span
                        className="home-task-list__spinner"
                        aria-hidden="true"
                      />
                    ) : (
                      <Circle aria-hidden="true" />
                    )}
                  </button>
                  <div>
                    <strong>{task.title}</strong>
                    {task.dueAt && <span>{dueLabel(task.dueAt)}</span>}
                  </div>
                </li>
              ))}
            </ul>
          ) : (
            <EmptyPanel
              title={copy.home.taskEmptyTitle}
              description={copy.home.taskEmptyDescription}
            />
          )}
        </section>
      </div>

      <section className="home-next-action" aria-labelledby="next-action-title">
        <div>
          <span className="home-next-action__icon" aria-hidden="true">
            <CheckCircle2 />
          </span>
          <div>
            <p>{copy.home.nextActionLabel}</p>
            <h2 id="next-action-title">
              {nextSchedule
                ? copy.home.nextActionSchedule(nextSchedule.title)
                : copy.home.nextActionEmpty}
            </h2>
          </div>
        </div>
        <button
          className="quiet-button focus-visible-control"
          type="button"
          onClick={onOpenAssistant}
        >
          {copy.home.openAssistant}
          <ArrowUpRight aria-hidden="true" />
        </button>
      </section>

      <aside
        className="home-mobile-assistant"
        aria-label={copy.home.assistantRailTitle}
      >
        <AssistantRail
          assistantReady={assistantReady}
          conversations={conversations}
          onOpenAssistant={onOpenAssistant}
        />
      </aside>
    </section>
  );
}

export function AssistantRail({
  assistantReady,
  conversations,
  onOpenAssistant,
}: Pick<
  HomeWorkspaceProps,
  "assistantReady" | "conversations" | "onOpenAssistant"
>) {
  const recent = conversations.slice(0, 3);
  return (
    <div className="assistant-rail">
      <div className="assistant-rail__identity">
        <span aria-hidden="true">
          <Sparkles />
        </span>
        <div>
          <strong>{copy.home.assistantRailTitle}</strong>
          <p>
            {assistantReady
              ? copy.home.assistantReady
              : copy.home.assistantNeedsConnection}
          </p>
        </div>
      </div>
      <button
        className="assistant-rail__prompt focus-visible-control"
        type="button"
        onClick={onOpenAssistant}
      >
        <span>{copy.home.assistantPrompt}</span>
        <ArrowUpRight aria-hidden="true" />
      </button>
      {recent.length ? (
        <div className="assistant-rail__recent">
          <p>{copy.home.recentConversations}</p>
          <ul>
            {recent.map((conversation) => (
              <li key={conversation.id}>
                {conversation.title ?? copy.conversations.untitled}
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <p className="assistant-rail__empty">{copy.home.recentEmpty}</p>
      )}
    </div>
  );
}

function PanelHeading({
  icon,
  title,
  meta,
}: {
  icon: ReactNode;
  title: string;
  meta: string;
}) {
  return (
    <header className="home-panel__heading">
      <div>
        {icon}
        <h2>{title}</h2>
      </div>
      <span>{meta}</span>
    </header>
  );
}

function EmptyPanel({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className="home-empty-panel">
      <strong>{title}</strong>
      <p>{description}</p>
    </div>
  );
}

function LoadingRows({ rows }: { rows: number }) {
  return (
    <div className="home-loading-rows" aria-label={copy.home.loadingShort}>
      {Array.from({ length: rows }, (_, index) => (
        <span key={index} className="skeleton" />
      ))}
    </div>
  );
}

function greetingForHour(hour: number): string {
  if (hour < 12) return copy.home.morningGreeting;
  if (hour < 18) return copy.home.afternoonGreeting;
  return copy.home.eveningGreeting;
}

function briefingTitle(
  loading: boolean,
  nextSchedule: HomeSnapshot["schedule"][number] | undefined,
  scheduleCount: number,
): string {
  if (loading) return copy.home.loadingBriefing;
  if (nextSchedule) return copy.home.briefingWithNext(nextSchedule.title);
  if (scheduleCount) return copy.home.briefingWithSchedule(scheduleCount);
  return copy.home.briefingEmpty;
}

function briefingDescription(
  loading: boolean,
  nextSchedule: HomeSnapshot["schedule"][number] | undefined,
  taskCount: number,
): string {
  if (loading) return copy.home.loadingDescription;
  if (nextSchedule) return copy.home.briefingTaskCount(taskCount);
  return taskCount
    ? copy.home.briefingOnlyTasks(taskCount)
    : copy.home.briefingNoItems;
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function scheduleDetail(
  startsAt: string,
  endsAt: string,
  notes: string | null,
): string {
  const time = `${formatTime(startsAt)}–${formatTime(endsAt)}`;
  return notes ? `${time} · ${notes}` : time;
}

function dueLabel(value: string): string {
  const due = new Date(value);
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  }).format(due);
}
