import { useMobileLayout } from "../useMobileLayout";
import { MobileTaskQueue } from "./MobileTaskQueue";
import { MobileDisclosure } from "./MobileDisclosure";
import { TaskSelectionControl } from "./TaskSelectionControl";
import {
  AlertTriangle,
  CalendarDays,
  CheckCircle2,
  ChevronRight,
  Circle,
  Cloud,
  Clock3,
  ListTodo,
  Mic,
  Pencil,
  Sparkles,
} from "lucide-react";
import {
  type Dispatch,
  type ReactNode,
  type SetStateAction,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { AgentJob, ConversationMessage } from "../api/agent";
import { type HomeSnapshot, type Recommendation } from "../api/home";
import { type ScheduleEntry, type Task } from "../api/planning";
import {
  type Project,
  type WeeklyProjectReport,
  type WeeklyReport,
} from "../api/projects";
import { type ProjectInflowItem } from "../api/googleChat";
import { type GmailInflowCandidate } from "../api/gmailInflow";
import { copy } from "../copy";
import { workspaceActionCopy } from "../copy/workspaceActions";
import {
  homeBriefPeriodForHour,
  type HomeBriefPeriod,
} from "../homeBriefPeriod";
import { upcomingHomeSchedules } from "../homeSchedule";
import {
  deadlineAttentionTasks,
  taskDueState,
  type TaskDueState,
} from "../planningDue";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import { HomeChatEntry, type HomeChatDraft } from "./HomeChatEntry";
import { HomeInflowReview, type MarkInflowsSeen } from "./HomeInflowReview";
import {
  GmailInflowReview,
  type PromoteGmailInflowInput,
} from "./GmailInflowReview";
import { type PromoteInflowInput } from "./ProjectInflowPanel";
import { filterMobileTasks } from "../mobileTaskView";
import { DeliveryAttention } from "./DeliveryAttention";

type HomeWorkspaceProps = {
  onRequestOpenHandled?(id: number): void;
  requestOpenRequest?: number;
  scheduledWork?: ReactNode;
  completionReviews?: ReactNode;
  highlightedCompletionReviewId?: string;
  deliveryFailureCount: number;
  onOpenDeliveryHistory(): void;
  suppressDecisionSuggestions?: boolean;
  onLoadAllTasks?(): Promise<Task[]>;
  snapshot: HomeSnapshot | undefined;
  loading: boolean;
  error: string | undefined;
  assistantReady: boolean;
  homeChatDraft: HomeChatDraft;
  setHomeChatDraft: Dispatch<SetStateAction<HomeChatDraft>>;
  onOpenConversationHistory(): void;
  onOpenAssistant(): void;
  onOpenPlanning(): void;
  onSendAssistant(text: string, clientMessageId: string): Promise<boolean>;
  onCompleteTask(task: Task): Promise<void>;
  onLoadAssistantTask(task: Pick<Task, "id" | "projectId">): Promise<Task>;
  onEditTask(task: Task): void;
  onEditSchedule(entry: ScheduleEntry): void;
  onOpenPlanningTask(task: Task): void | Promise<void>;
  onOpenTask(task: Pick<Task, "id" | "projectId">): void | Promise<void>;
  onOpenProject(
    project: Pick<Project, "id" | "workspaceId">,
  ): void | Promise<void>;
  onOpenSchedule(
    entry: Pick<ScheduleEntry, "id" | "startsAt">,
  ): void | Promise<void>;
  onOpenDecisionInbox(): void;
  onOpenMeetings(): void;
  onOpenSettings(): void;
  onDecideRecommendation(
    recommendation: Recommendation,
    decision: "approve" | "defer",
  ): Promise<boolean>;
  inflowSaving: boolean;
  onPromoteInflow(
    item: ProjectInflowItem,
    input: PromoteInflowInput,
  ): Promise<void>;
  onDismissInflow(
    item: ProjectInflowItem,
    input?: { reason?: string; replyToSource?: boolean; markSeen?: boolean },
  ): Promise<void>;
  onRetryInflowAnalysis(item: ProjectInflowItem): Promise<void>;
  onMarkInflowsSeen?: MarkInflowsSeen;
  onReviewInflowChange?(item: ProjectInflowItem): void;
  gmailInflowItems: GmailInflowCandidate[];
  gmailInflowProjects: Project[];
  gmailInflowLoading: boolean;
  gmailInflowLoadingMore: boolean;
  gmailInflowLoadMoreError: boolean;
  gmailInflowHasMore: boolean;
  gmailInflowError: string | undefined;
  gmailInflowSavingId: string | undefined;
  onReloadGmailInflow(): void | Promise<void>;
  onLoadMoreGmailInflow(): void | Promise<void>;
  onPromoteGmailInflow(
    candidate: GmailInflowCandidate,
    input: PromoteGmailInflowInput,
  ): Promise<void>;
  onDismissGmailInflow(candidate: GmailInflowCandidate): Promise<void>;
  onDeferGmailInflow(
    candidate: GmailInflowCandidate,
    revisitAt: string,
  ): Promise<void>;
  onRetryGmailInflowAnalysis(candidate: GmailInflowCandidate): Promise<void>;
};

export function HomeWorkspace({
  requestOpenRequest,
  onRequestOpenHandled,
  scheduledWork,
  completionReviews,
  highlightedCompletionReviewId,
  deliveryFailureCount,
  onOpenDeliveryHistory,
  suppressDecisionSuggestions = false,
  onLoadAllTasks,
  snapshot,
  loading,
  error,
  assistantReady,
  homeChatDraft,
  setHomeChatDraft,
  onOpenConversationHistory,
  onOpenAssistant,
  onOpenPlanning,
  onSendAssistant,
  onCompleteTask,
  onLoadAssistantTask,
  onEditTask,
  onEditSchedule,
  onOpenPlanningTask,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
  onDecideRecommendation,
  inflowSaving,
  onPromoteInflow,
  onDismissInflow,
  onMarkInflowsSeen,
  onRetryInflowAnalysis,
  onReviewInflowChange,
  gmailInflowItems,
  gmailInflowProjects,
  gmailInflowLoading,
  gmailInflowLoadingMore,
  gmailInflowLoadMoreError,
  gmailInflowHasMore,
  gmailInflowError,
  gmailInflowSavingId,
  onReloadGmailInflow,
  onLoadMoreGmailInflow,
  onPromoteGmailInflow,
  onDismissGmailInflow,
  onDeferGmailInflow,
  onRetryGmailInflowAnalysis,
}: HomeWorkspaceProps) {
  const mobile = useMobileLayout();
  const [completingTaskId, setCompletingTaskId] = useState<string>();
  const [selectedTaskIds, setSelectedTaskIds] = useState<string[]>([]);
  const [highlightedHomeTaskId, setHighlightedHomeTaskId] = useState<string>();
  const [overviewFocusTarget, setOverviewFocusTarget] = useState<
    "schedule" | "tasks"
  >();
  const [scheduleNow, setScheduleNow] = useState(() => new Date());
  const [todayRequest, setTodayRequest] = useState(0);
  const [recommendationAnnouncement, setRecommendationAnnouncement] =
    useState("");
  const highlightedHomeTaskRef = useRef<HTMLLIElement | null>(null);
  const scheduleSectionRef = useRef<HTMLElement | null>(null);
  const taskSectionRef = useRef<HTMLElement | null>(null);
  const greeting = useMemo(() => greetingForHour(new Date().getHours()), []);
  const initialLoading = loading && snapshot === undefined;
  const skeletonVisible = useDelayedSkeleton(initialLoading);
  const showingSkeleton = initialLoading || skeletonVisible;
  const upcomingSchedule = useMemo(
    () => upcomingHomeSchedules(snapshot?.schedule ?? [], scheduleNow),
    [scheduleNow, snapshot?.schedule],
  );
  const nextSchedule = upcomingSchedule[0];
  const scheduleCount = upcomingSchedule.length;
  const taskCount = snapshot?.tasks.length ?? 0;
  const todayTasks = useMemo(
    () =>
      filterMobileTasks(
        [
          ...new Map(
            [...(snapshot?.tasks ?? []), ...(snapshot?.dueTasks ?? [])].map(
              (task) => [task.id, task] as const,
            ),
          ).values(),
        ],
        "today",
        scheduleNow,
      ),
    [snapshot?.tasks, snapshot?.dueTasks, scheduleNow],
  );
  const briefPeriod = homeBriefPeriodForHour(scheduleNow.getHours());
  const dueTasks = useMemo(
    () => deadlineAttentionTasks(snapshot?.dueTasks ?? []),
    [snapshot?.dueTasks],
  );
  const weeklyPriorityTasks = useMemo(
    () => selectWeeklyPriorityTasks(snapshot?.weeklyReports ?? [], dueTasks),
    [dueTasks, snapshot?.weeklyReports],
  );
  const remainingDueTasks = useMemo(() => {
    const priorityTaskIds = new Set(weeklyPriorityTasks.map((task) => task.id));
    return dueTasks.filter((task) => !priorityTaskIds.has(task.id));
  }, [dueTasks, weeklyPriorityTasks]);
  const showSchedulePanel = showingSkeleton || Boolean(nextSchedule);
  const showTaskPanel = showingSkeleton || taskCount > 0;
  const overviewPanelCount = Number(showSchedulePanel) + Number(showTaskPanel);

  useEffect(() => {
    setScheduleNow(new Date());
  }, [snapshot?.schedule]);

  useEffect(() => {
    const nextEnd = upcomingSchedule.reduce<number | undefined>(
      (nearest, entry) => {
        const endsAt = new Date(entry.endsAt).getTime();
        if (!Number.isFinite(endsAt)) return nearest;
        return nearest === undefined ? endsAt : Math.min(nearest, endsAt);
      },
      undefined,
    );
    if (nextEnd === undefined) return;
    const timeout = window.setTimeout(
      () => setScheduleNow(new Date()),
      Math.max(50, nextEnd - scheduleNow.getTime() + 50),
    );
    return () => window.clearTimeout(timeout);
  }, [scheduleNow, upcomingSchedule]);

  useEffect(() => {
    if (!overviewFocusTarget) return;
    const element =
      overviewFocusTarget === "tasks"
        ? (highlightedHomeTaskRef.current ?? taskSectionRef.current)
        : scheduleSectionRef.current;
    if (!element) return;
    element.scrollIntoView({ block: "start" });
    element.focus({ preventScroll: true });
    setOverviewFocusTarget(undefined);
  }, [overviewFocusTarget]);

  useEffect(() => {
    const available = new Set(snapshot?.tasks.map((task) => task.id) ?? []);
    setSelectedTaskIds((ids) => ids.filter((id) => available.has(id)));
  }, [snapshot?.tasks]);

  async function completeSelectedTasks() {
    if (completingTaskId) return;
    const tasks =
      snapshot?.tasks.filter((task) => selectedTaskIds.includes(task.id)) ?? [];
    try {
      for (const task of tasks) {
        setCompletingTaskId(task.id);
        await onCompleteTask(task);
      }
    } finally {
      setCompletingTaskId(undefined);
    }
  }

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
    <section
      className="home-page"
      data-assistant-state="overview"
      data-overview-panels={overviewPanelCount}
      aria-busy={loading}
    >
      <header className="home-greeting">
        <div>
          <h1 id="home-overview-title" tabIndex={-1}>
            {greeting}
          </h1>
          <p>{copy.home.title}</p>
        </div>
        <div className="home-quick-actions">
          {scheduledWork && (
            <div className="home-scheduled-work">{scheduledWork}</div>
          )}
          {snapshot && onLoadAllTasks && (
            <button
              type="button"
              className="home-today-shortcut focus-visible-control"
              aria-controls="home-today-task-list"
              onClick={() => {
                setTodayRequest((previous) => previous + 1);
                setHighlightedHomeTaskId(undefined);
                setOverviewFocusTarget(undefined);
              }}
            >
              <ListTodo aria-hidden="true" />
              <span>
                <strong>
                  {workspaceActionCopy.todayTasks(todayTasks.length)}
                </strong>
                <small>
                  {todayTasks[0]?.title ?? workspaceActionCopy.noTodayTasks}
                </small>
              </span>
              <ChevronRight aria-hidden="true" />
            </button>
          )}
        </div>
      </header>

      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}
      <p className="sr-only" aria-live="polite">
        {recommendationAnnouncement}
      </p>

      {!showingSkeleton && (snapshot || completionReviews) && (
        <div className="home-inflow-primary">
          <HomeInflowReview
            focusRequest={requestOpenRequest}
            onFocusHandled={onRequestOpenHandled}
            initialView="list"
            items={snapshot?.inflow ?? []}
            completionReviews={completionReviews}
            highlightedCompletionReviewId={highlightedCompletionReviewId}
            saving={inflowSaving}
            onPromote={onPromoteInflow}
            onDismiss={onDismissInflow}
            onMarkSeenMany={onMarkInflowsSeen}
            onRetryAnalysis={onRetryInflowAnalysis}
            onOpenDeliveryHistory={onOpenDeliveryHistory}
            onReviewChange={onReviewInflowChange}
            onOpenTask={async (taskId) => {
              const task = await onLoadAssistantTask({
                id: taskId,
                projectId: null,
              });
              await onOpenTask(task);
              if (!task.projectId) {
                setHighlightedHomeTaskId(task.id);
                setOverviewFocusTarget("tasks");
              }
            }}
          />
        </div>
      )}

      {onLoadAllTasks && (
        <MobileTaskQueue
          sectionRef={taskSectionRef}
          todayRequest={todayRequest}
          tasks={snapshot?.tasks ?? []}
          loading={loading}
          onLoadAll={onLoadAllTasks}
          onComplete={onCompleteTask}
          onEdit={onEditTask}
        />
      )}

      <div className="home-assistant-entry">
        <HomeChatEntry
          ready={assistantReady}
          draft={homeChatDraft}
          setDraft={setHomeChatDraft}
          onOpenHistory={onOpenConversationHistory}
          onSend={onSendAssistant}
        />
      </div>

      <DeliveryAttention
        count={deliveryFailureCount}
        onOpen={onOpenDeliveryHistory}
      />

      {mobile && nextSchedule && (
        <section className="mobile-home-next" aria-label="다음 일정">
          <h2>다음 일정</h2>
          <ScheduleHighlight
            entry={nextSchedule}
            onOpen={() => void onOpenSchedule(nextSchedule)}
            onEdit={
              nextSchedule.source === "manual"
                ? () => onEditSchedule(nextSchedule)
                : undefined
            }
          />
        </section>
      )}

      <>
        <MobileDisclosure
          mobile
          title={`들어온 메일 · ${gmailInflowItems.length}개`}
        >
          <GmailInflowReview
            items={gmailInflowItems}
            projects={gmailInflowProjects}
            loading={gmailInflowLoading}
            loadingMore={gmailInflowLoadingMore}
            loadMoreError={gmailInflowLoadMoreError}
            hasMore={gmailInflowHasMore}
            error={gmailInflowError}
            savingId={gmailInflowSavingId}
            onReload={onReloadGmailInflow}
            onLoadMore={onLoadMoreGmailInflow}
            onPromote={onPromoteGmailInflow}
            onDismiss={onDismissGmailInflow}
            onDefer={onDeferGmailInflow}
            onRetryAnalysis={onRetryGmailInflowAnalysis}
            onOpenTask={async (taskId) => {
              const task = await onLoadAssistantTask({
                id: taskId,
                projectId: null,
              });
              await onOpenTask(task);
              if (!task.projectId) {
                setHighlightedHomeTaskId(task.id);
                setOverviewFocusTarget("tasks");
              }
            }}
          />
        </MobileDisclosure>

        {!showingSkeleton && Boolean(snapshot?.weeklyReports.length) && (
          <MobileDisclosure mobile title="이번 주 운영 리포트">
            <WeeklyOperationsBrief
              reports={snapshot?.weeklyReports ?? []}
              priorityTasks={weeklyPriorityTasks}
              completingTaskId={completingTaskId}
              onCompleteTask={complete}
              onEditTask={onEditTask}
              onOpenTask={onOpenPlanningTask}
              onOpenProject={onOpenProject}
            />
          </MobileDisclosure>
        )}

        {!mobile &&
          !onLoadAllTasks &&
          !showingSkeleton &&
          remainingDueTasks.length > 0 && (
            <DeadlineBrief
              tasks={remainingDueTasks}
              completingTaskId={completingTaskId}
              onCompleteTask={complete}
              onEditTask={onEditTask}
              onOpenTask={onOpenPlanningTask}
            />
          )}

        {!suppressDecisionSuggestions &&
          !showingSkeleton &&
          Boolean(snapshot?.recommendations.length) && (
            <MobileDisclosure mobile={mobile} title="결정할 일">
              <NowBrief
                recommendations={snapshot?.recommendations ?? []}
                period={briefPeriod}
                onDecide={onDecideRecommendation}
                onOpenTask={onOpenTask}
                onOpenProject={onOpenProject}
                onAnnounce={setRecommendationAnnouncement}
              />
            </MobileDisclosure>
          )}

        {!mobile && !onLoadAllTasks && (
          <button
            className="home-briefing focus-visible-control"
            type="button"
            onClick={onOpenPlanning}
            aria-label={copy.home.openPlanning}
          >
            {showingSkeleton ? (
              <HomeBriefingSkeleton visible={skeletonVisible} />
            ) : (
              <>
                <span className="home-briefing__symbol" aria-hidden="true">
                  <Sparkles />
                </span>
                <span className="home-briefing__copy">
                  <strong>
                    {briefingHeading(nextSchedule, scheduleCount)}
                  </strong>
                  <span>{briefingSummary(scheduleCount, taskCount)}</span>
                </span>
                <ChevronRight aria-hidden="true" />
              </>
            )}
          </button>
        )}

        {!mobile && !onLoadAllTasks && (
          <button
            className="home-voice-callout focus-visible-control"
            type="button"
            onClick={onOpenAssistant}
          >
            <span className="home-voice-callout__icon" aria-hidden="true">
              <Mic />
            </span>
            <span>
              <strong>{copy.home.askAssistant}</strong>
              <span>{copy.home.description}</span>
            </span>
            <ChevronRight aria-hidden="true" />
          </button>
        )}

        {!mobile && showSchedulePanel && (
          <section
            className="home-next-schedule"
            aria-labelledby="next-schedule-title"
            ref={scheduleSectionRef}
            tabIndex={-1}
          >
            <div className="home-section-heading">
              <h2 id="next-schedule-title">
                {nextSchedule ? "다음 일정" : copy.home.scheduleTitle}
              </h2>
              {nextSchedule && (
                <span>{relativeScheduleTime(nextSchedule.startsAt)}</span>
              )}
            </div>
            <div className="home-schedule-surface">
              {showingSkeleton ? (
                <ScheduleSkeleton visible={skeletonVisible} />
              ) : nextSchedule ? (
                <ScheduleHighlight
                  entry={nextSchedule}
                  onOpen={() => void onOpenSchedule(nextSchedule)}
                  onEdit={
                    nextSchedule.source === "manual"
                      ? () => onEditSchedule(nextSchedule)
                      : undefined
                  }
                />
              ) : null}
            </div>
          </section>
        )}

        {!mobile && !onLoadAllTasks && showTaskPanel && (
          <section
            className="home-tasks"
            aria-labelledby="today-task-title"
            ref={taskSectionRef}
            tabIndex={-1}
          >
            <div className="home-section-heading">
              <h2 id="today-task-title">{copy.home.taskTitle}</h2>
              <span>
                {showingSkeleton ? (
                  <SkeletonGroup
                    className="skeleton-count"
                    label={copy.home.loadingShort}
                    visible={skeletonVisible}
                  >
                    <SkeletonBlock />
                  </SkeletonGroup>
                ) : (
                  copy.home.taskCount(taskCount)
                )}
              </span>
            </div>
            <div className="home-task-surface">
              {showingSkeleton ? (
                <TaskListSkeleton rows={4} visible={skeletonVisible} />
              ) : snapshot?.tasks.length ? (
                <ul className="home-task-list">
                  {snapshot.tasks.map((task, index) => (
                    <li
                      key={task.id}
                      data-mobile-overflow={index >= 3}
                      ref={
                        highlightedHomeTaskId === task.id
                          ? highlightedHomeTaskRef
                          : undefined
                      }
                      data-highlighted={highlightedHomeTaskId === task.id}
                      data-selected={selectedTaskIds.includes(task.id)}
                      tabIndex={
                        highlightedHomeTaskId === task.id ? -1 : undefined
                      }
                    >
                      <button
                        className="home-task-list__complete focus-visible-control"
                        type="button"
                        role="checkbox"
                        aria-checked={selectedTaskIds.includes(task.id)}
                        onClick={() =>
                          setSelectedTaskIds((ids) =>
                            ids.includes(task.id)
                              ? ids.filter((id) => id !== task.id)
                              : [...ids, task.id],
                          )
                        }
                        disabled={Boolean(completingTaskId)}
                        aria-label={`${task.title} 선택`}
                      >
                        {completingTaskId === task.id ? (
                          <span className="button-spinner" aria-hidden="true" />
                        ) : selectedTaskIds.includes(task.id) ? (
                          <CheckCircle2 aria-hidden="true" />
                        ) : (
                          <Circle aria-hidden="true" />
                        )}
                      </button>
                      <button
                        className="home-task-list__open focus-visible-control"
                        type="button"
                        onClick={() => void onOpenPlanningTask(task)}
                        disabled={Boolean(completingTaskId)}
                        aria-label={copy.home.openTaskInSchedule(task.title)}
                      >
                        <span className="home-task-list__content">
                          <strong>{task.title} </strong>
                          <small data-assigned={Boolean(task.assigneeName)}>
                            {copy.projects.taskAssignee(
                              task.assigneeName ?? undefined,
                            )}
                          </small>
                        </span>
                        {task.dueAt && (
                          <time
                            dateTime={task.dueAt}
                            data-due-state={taskDueState(task)}
                          >
                            {taskDueLabel(task)}
                          </time>
                        )}
                      </button>
                      <button
                        className="home-task-list__edit focus-visible-control"
                        type="button"
                        onClick={() => onEditTask(task)}
                        disabled={Boolean(completingTaskId)}
                        aria-label={copy.home.editTask(task.title)}
                      >
                        <Pencil aria-hidden="true" />
                      </button>
                    </li>
                  ))}
                </ul>
              ) : null}
              {selectedTaskIds.length > 0 && (
                <div className="home-task-selection">
                  <span role="status">{selectedTaskIds.length}개 선택됨</span>
                  <button
                    className="primary-button focus-visible-control"
                    type="button"
                    disabled={Boolean(completingTaskId)}
                    onClick={() => void completeSelectedTasks()}
                  >
                    {completingTaskId
                      ? "완료 처리 중…"
                      : "선택한 할 일 완료하기"}
                  </button>
                </div>
              )}
              {!showingSkeleton && taskCount > 3 && (
                <button
                  className="home-tasks__mobile-more focus-visible-control"
                  type="button"
                  onClick={onOpenPlanning}
                >
                  {copy.home.showMoreTasks(taskCount - 3)}
                  <ChevronRight aria-hidden="true" />
                </button>
              )}
            </div>
          </section>
        )}
      </>
    </section>
  );
}

function NowBrief({
  recommendations,
  period,
  onDecide,
  onOpenTask,
  onOpenProject,
  onAnnounce,
}: {
  recommendations: Recommendation[];
  period: HomeBriefPeriod;
  onDecide(
    recommendation: Recommendation,
    decision: "approve" | "defer",
  ): Promise<boolean>;
  onOpenTask(task: Pick<Task, "id" | "projectId">): void | Promise<void>;
  onOpenProject(
    project: Pick<Project, "id" | "workspaceId">,
  ): void | Promise<void>;
  onAnnounce(message: string): void;
}) {
  const [pendingId, setPendingId] = useState<string>();
  const [error, setError] = useState<string>();
  const [mobileExpanded, setMobileExpanded] = useState(false);

  async function decide(
    recommendation: Recommendation,
    decision: "approve" | "defer",
  ) {
    if (pendingId) return;
    setPendingId(recommendation.id);
    setError(undefined);
    const succeeded = await onDecide(recommendation, decision);
    setPendingId(undefined);
    if (!succeeded) {
      setError(copy.home.recommendationDecisionNotice);
      return;
    }
    onAnnounce(
      decision === "defer"
        ? copy.home.recommendationDeferred
        : copy.home.recommendationConfirmed,
    );
  }

  function openRelated(recommendation: Recommendation) {
    if (!recommendation.suggestedEntityId) return;
    if (
      recommendation.projectId === recommendation.suggestedEntityId &&
      recommendation.workspaceId
    ) {
      void onOpenProject({
        id: recommendation.suggestedEntityId,
        workspaceId: recommendation.workspaceId,
      });
      return;
    }
    void onOpenTask({
      id: recommendation.suggestedEntityId,
      projectId: recommendation.projectId,
    });
  }

  return (
    <section
      className="home-now-brief"
      aria-labelledby="home-now-brief-title"
      data-mobile-expanded={mobileExpanded}
    >
      <header>
        <div>
          <span>{nowBriefHeading(period).eyebrow}</span>
          <h2 id="home-now-brief-title">{nowBriefHeading(period).title}</h2>
        </div>
        <strong>{copy.home.nowBriefCount(recommendations.length)}</strong>
      </header>
      {error && (
        <p className="home-now-brief__error" role="alert">
          {error}
        </p>
      )}
      <ol>
        {recommendations.map((recommendation, index) => {
          const pending = pendingId === recommendation.id;
          return (
            <li key={recommendation.id} data-mobile-overflow={index >= 2}>
              <span className="home-now-brief__marker" aria-hidden="true">
                <Sparkles />
              </span>
              <div className="home-now-brief__content">
                <h3>{recommendation.title}</h3>
                <p>{recommendation.rationale}</p>
                <dl>
                  <div>
                    <dt>{copy.home.recommendationEffect}</dt>
                    <dd>{recommendation.expectedEffect}</dd>
                  </div>
                  {recommendation.riskSummary && (
                    <div>
                      <dt>{copy.home.recommendationRisk}</dt>
                      <dd>{recommendation.riskSummary}</dd>
                    </div>
                  )}
                </dl>
              </div>
              <div className="home-now-brief__actions">
                {recommendation.suggestedEntityId && (
                  <button
                    className="home-now-brief__source-action secondary-button focus-visible-control"
                    type="button"
                    disabled={Boolean(pendingId)}
                    onClick={() => openRelated(recommendation)}
                  >
                    {copy.home.openRecommendationSource}
                  </button>
                )}
                <button
                  className="secondary-button focus-visible-control"
                  type="button"
                  disabled={Boolean(pendingId)}
                  onClick={() => void decide(recommendation, "defer")}
                >
                  {copy.home.recommendationDefer}
                </button>
                <button
                  className="primary-button focus-visible-control"
                  type="button"
                  disabled={Boolean(pendingId)}
                  onClick={() => void decide(recommendation, "approve")}
                >
                  {pending && (
                    <span className="button-spinner" aria-hidden="true" />
                  )}
                  {copy.home.recommendationConfirm}
                </button>
              </div>
            </li>
          );
        })}
      </ol>
      {recommendations.length > 2 && (
        <button
          className="home-now-brief__mobile-toggle focus-visible-control"
          type="button"
          aria-expanded={mobileExpanded}
          onClick={() => setMobileExpanded((current) => !current)}
        >
          {mobileExpanded
            ? copy.home.collapseRecommendations
            : copy.home.showMoreRecommendations(recommendations.length - 2)}
        </button>
      )}
    </section>
  );
}

type WeeklyOperationProject = WeeklyProjectReport & {
  workspaceId: string;
};

function WeeklyOperationsBrief({
  reports,
  priorityTasks,
  completingTaskId,
  onCompleteTask,
  onEditTask,
  onOpenTask,
  onOpenProject,
}: {
  reports: WeeklyReport[];
  priorityTasks: Task[];
  completingTaskId: string | undefined;
  onCompleteTask(task: Task): Promise<void>;
  onEditTask(task: Task): void;
  onOpenTask(task: Task): void | Promise<void>;
  onOpenProject(
    project: Pick<Project, "id" | "workspaceId">,
  ): void | Promise<void>;
}) {
  const activeReports = reports.filter((report) => report.projects.length > 0);
  if (activeReports.length === 0) return null;

  const totals = activeReports.reduce(
    (current, report) => ({
      created: current.created + report.createdTaskCount,
      completed: current.completed + report.completedTaskCount,
      backlog: current.backlog + report.backlogDelta,
      overdue: current.overdue + report.overdueTaskCount,
      stale: current.stale + report.staleTaskCount,
      chatAttention:
        current.chatAttention + (report.actionableChatInflowCount ?? 0),
      gmailAttention:
        current.gmailAttention + (report.actionableGmailInflowCount ?? 0),
    }),
    {
      created: 0,
      completed: 0,
      backlog: 0,
      overdue: 0,
      stale: 0,
      chatAttention: 0,
      gmailAttention: 0,
    },
  );
  const attentionProjects = weeklyAttentionProjects(activeReports);

  return (
    <section
      className="home-weekly-operations"
      aria-labelledby="home-weekly-operations-title"
    >
      <header>
        <span className="home-weekly-operations__icon" aria-hidden="true">
          <ListTodo />
        </span>
        <div>
          <span>{copy.home.weeklyOperationsEyebrow}</span>
          <h2 id="home-weekly-operations-title">
            {copy.home.weeklyOperationsTitle}
          </h2>
          <p>
            {copy.home.weeklyOperationsSummary(
              totals.created,
              totals.completed,
              totals.backlog,
            )}
          </p>
        </div>
      </header>

      <dl className="home-weekly-operations__metrics">
        <WeeklyOperationMetric
          label={copy.home.weeklyNewWork}
          value={totals.created}
        />
        <WeeklyOperationMetric
          label={copy.home.weeklyCompletedWork}
          value={totals.completed}
        />
        <WeeklyOperationMetric
          label={copy.home.weeklyOverdueWork}
          value={totals.overdue}
          attention={totals.overdue > 0}
        />
        <WeeklyOperationMetric
          label={copy.home.weeklyStaleWork}
          value={totals.stale}
          attention={totals.stale > 0}
        />
        <WeeklyOperationMetric
          label={copy.home.weeklyChatAttention}
          value={totals.chatAttention}
          attention={totals.chatAttention > 0}
        />
        <WeeklyOperationMetric
          label={copy.home.weeklyGmailAttention}
          value={totals.gmailAttention}
          attention={totals.gmailAttention > 0}
        />
      </dl>

      <section
        className="home-weekly-operations__priority"
        aria-labelledby="home-weekly-priority-title"
      >
        <header>
          <div>
            <h3 id="home-weekly-priority-title">
              {copy.home.weeklyPriorityTitle}
            </h3>
            <p>{copy.home.weeklyPriorityDescription}</p>
          </div>
        </header>
        {priorityTasks.length > 0 ? (
          <ul>
            {priorityTasks.map((task) => (
              <li key={task.id}>
                <TaskSelectionControl
                  title={task.title}
                  className="home-weekly-operations__complete focus-visible-control"
                  disabled={Boolean(completingTaskId)}
                  busy={completingTaskId === task.id}
                  onComplete={() => onCompleteTask(task)}
                />
                <button
                  className="home-weekly-operations__task focus-visible-control"
                  type="button"
                  onClick={() => void onOpenTask(task)}
                >
                  <span>
                    <strong>{task.title}</strong>
                    <small>
                      {copy.projects.taskAssignee(
                        task.assigneeName ?? undefined,
                      )}
                    </small>
                  </span>
                  {task.dueAt && (
                    <time dateTime={task.dueAt}>
                      {formatDueTime(task.dueAt)}
                    </time>
                  )}
                </button>
                <button
                  className="home-weekly-operations__edit focus-visible-control"
                  type="button"
                  onClick={() => onEditTask(task)}
                  aria-label={copy.home.editTask(task.title)}
                >
                  <Pencil aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="home-weekly-operations__priority-empty">
            {copy.home.weeklyPriorityEmpty}
          </p>
        )}
      </section>

      {attentionProjects.length > 0 ? (
        <>
          <WeeklyProjectLinks
            className="home-weekly-operations__projects"
            projects={attentionProjects}
            onOpenProject={onOpenProject}
          />
          <details className="home-weekly-operations__projects-mobile">
            <summary className="focus-visible-control">
              <span>
                <strong>
                  {copy.home.weeklyProjectsNeedAttention(
                    attentionProjects.length,
                  )}
                </strong>
                <small>
                  {copy.home.weeklyProjectsNeedAttentionDescription}
                </small>
              </span>
              <ChevronRight aria-hidden="true" />
            </summary>
            <WeeklyProjectLinks
              projects={attentionProjects}
              onOpenProject={onOpenProject}
            />
          </details>
        </>
      ) : (
        <p className="home-weekly-operations__clear">
          <CheckCircle2 aria-hidden="true" />
          <span>{copy.home.weeklyOperationsClear}</span>
        </p>
      )}
    </section>
  );
}

function WeeklyProjectLinks({
  className,
  projects,
  onOpenProject,
}: {
  className?: string;
  projects: WeeklyOperationProject[];
  onOpenProject(
    project: Pick<Project, "id" | "workspaceId">,
  ): void | Promise<void>;
}) {
  return (
    <ul className={className}>
      {projects.map((project) => (
        <li key={project.projectId}>
          <button
            className="focus-visible-control"
            type="button"
            onClick={() =>
              void onOpenProject({
                id: project.projectId,
                workspaceId: project.workspaceId,
              })
            }
          >
            <span>
              <strong>{project.title}</strong>
              <small>{weeklyProjectSummary(project)}</small>
            </span>
            <ChevronRight aria-hidden="true" />
          </button>
        </li>
      ))}
    </ul>
  );
}

function WeeklyOperationMetric({
  label,
  value,
  attention = false,
}: {
  label: string;
  value: number;
  attention?: boolean;
}) {
  return (
    <div data-attention={attention}>
      <dt>{label}</dt>
      <dd>{copy.home.weeklyMetricCount(value)}</dd>
    </div>
  );
}

function weeklyAttentionProjects(
  reports: WeeklyReport[],
): WeeklyOperationProject[] {
  return reports
    .flatMap((report) =>
      report.projects.map((project) => ({
        ...project,
        workspaceId: report.workspaceId,
      })),
    )
    .filter((project) => project.health !== "on_track")
    .sort(
      (left, right) =>
        weeklyProjectAttentionRank(right) - weeklyProjectAttentionRank(left) ||
        left.title.localeCompare(right.title, "ko"),
    )
    .slice(0, 4);
}

export function selectWeeklyPriorityTasks(
  reports: WeeklyReport[],
  tasks: Task[],
): Task[] {
  const attentionProjectIds = new Set(
    weeklyAttentionProjects(reports).map((project) => project.projectId),
  );
  return tasks
    .filter(
      (task) =>
        task.status === "open" &&
        Boolean(task.projectId) &&
        attentionProjectIds.has(task.projectId ?? ""),
    )
    .slice(0, 3);
}

function weeklyProjectAttentionRank(project: WeeklyOperationProject): number {
  return (
    project.overdueTaskCount * 100 +
    Math.max(0, project.backlogDelta) * 10 +
    project.staleTaskCount * 5 +
    project.unassignedTaskCount
  );
}

function weeklyProjectSummary(project: WeeklyOperationProject): string {
  if (project.overdueTaskCount > 0) {
    return copy.home.weeklyProjectOverdue(project.overdueTaskCount);
  }
  if (project.backlogDelta > 0) {
    return copy.home.weeklyProjectBacklog(project.backlogDelta);
  }
  if (project.staleTaskCount > 0) {
    return copy.home.weeklyProjectStale(project.staleTaskCount);
  }
  return copy.home.weeklyProjectUnassigned(project.unassignedTaskCount);
}

export function EmptySurface({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className="empty-surface">
      <Clock3 aria-hidden="true" />
      <div>
        <strong>{title}</strong>
        <p>{description}</p>
      </div>
    </div>
  );
}

function HomeBriefingSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="home-briefing-skeleton"
      label={copy.home.loadingDescription}
      visible={visible}
    >
      <SkeletonBlock className="skeleton--briefing-icon" />
      <span className="skeleton-copy-stack">
        <SkeletonBlock className="skeleton--title" />
        <SkeletonBlock className="skeleton--caption" />
      </span>
      <SkeletonBlock className="skeleton--chevron" />
    </SkeletonGroup>
  );
}

function ScheduleSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="schedule-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      <SkeletonBlock className="skeleton--schedule-icon" />
      <span className="skeleton-copy-stack">
        <SkeletonBlock className="skeleton--title" />
        <SkeletonBlock className="skeleton--caption" />
      </span>
      <SkeletonBlock className="skeleton--chevron" />
    </SkeletonGroup>
  );
}

function TaskListSkeleton({
  rows,
  visible,
}: {
  rows: number;
  visible: boolean;
}) {
  return (
    <SkeletonGroup
      className="task-list-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      {Array.from({ length: rows }, (_, index) => (
        <span className="task-row-skeleton" key={index}>
          <SkeletonBlock className="skeleton--task-control" />
          <SkeletonBlock className="skeleton--task-title" />
          <SkeletonBlock className="skeleton--task-date" />
        </span>
      ))}
    </SkeletonGroup>
  );
}

function ScheduleHighlight({
  entry,
  onOpen,
  onEdit,
}: {
  entry: ScheduleEntry;
  onOpen: () => void;
  onEdit?: () => void;
}) {
  return (
    <div className="schedule-highlight">
      <button
        className="schedule-highlight__open focus-visible-control"
        type="button"
        onClick={onOpen}
        aria-label={copy.home.openScheduleInSchedule(entry.title)}
      >
        <span className="schedule-highlight__icon" aria-hidden="true">
          <CalendarDays />
        </span>
        <span className="schedule-highlight__copy">
          <strong>{entry.title}</strong>
          <span>{scheduleDetail(entry)}</span>
        </span>
      </button>
      {onEdit ? (
        <button
          className="schedule-highlight__edit focus-visible-control"
          type="button"
          onClick={onEdit}
          aria-label={copy.schedule.editSchedule(entry.title)}
        >
          <Pencil aria-hidden="true" />
          <span>{copy.actions.edit}</span>
        </button>
      ) : (
        <span
          className="schedule-highlight__source"
          title={copy.schedule.connectedCalendar}
        >
          <Cloud aria-hidden="true" />
          <span className="sr-only">{copy.schedule.connectedCalendar}</span>
        </span>
      )}
    </div>
  );
}

function DeadlineBrief({
  tasks,
  completingTaskId,
  onCompleteTask,
  onEditTask,
  onOpenTask,
}: {
  tasks: Task[];
  completingTaskId: string | undefined;
  onCompleteTask(task: Task): Promise<void>;
  onEditTask(task: Task): void;
  onOpenTask(task: Task): void | Promise<void>;
}) {
  const [expanded, setExpanded] = useState(false);
  const overdueCount = tasks.filter(
    (task) => taskDueState(task) === "overdue",
  ).length;
  const upcomingCount = tasks.length - overdueCount;
  const visibleTasks = expanded ? tasks : tasks.slice(0, 4);
  const hiddenCount = Math.max(0, tasks.length - visibleTasks.length);
  return (
    <section
      className="home-deadline-brief"
      aria-labelledby="home-deadline-title"
    >
      <header>
        <span className="home-deadline-brief__icon" aria-hidden="true">
          <AlertTriangle />
        </span>
        <div>
          <h2 id="home-deadline-title">{copy.home.deadlineTitle}</h2>
          <p>{copy.home.deadlineSummary(overdueCount, upcomingCount)}</p>
        </div>
        <strong>{copy.home.deadlineCount(tasks.length)}</strong>
      </header>
      <ul>
        {visibleTasks.map((task) => {
          const state = taskDueState(task);
          return (
            <li key={task.id} data-due-state={state}>
              <TaskSelectionControl
                title={task.title}
                className="home-deadline-brief__complete focus-visible-control"
                disabled={Boolean(completingTaskId)}
                busy={completingTaskId === task.id}
                onComplete={() => onCompleteTask(task)}
              />
              <button
                className="home-deadline-brief__task focus-visible-control"
                type="button"
                onClick={() => void onOpenTask(task)}
              >
                <span className="home-deadline-brief__due-state">
                  {dueStateLabel(state)}
                </span>
                <span className="home-deadline-brief__copy">
                  <strong>{task.title}</strong>
                  <small data-assigned={Boolean(task.assigneeName)}>
                    {copy.projects.taskAssignee(task.assigneeName ?? undefined)}
                  </small>
                </span>
                {task.dueAt && (
                  <time dateTime={task.dueAt}>{formatDueTime(task.dueAt)}</time>
                )}
              </button>
              <button
                className="home-deadline-brief__edit focus-visible-control"
                type="button"
                onClick={() => onEditTask(task)}
                aria-label={copy.home.editTask(task.title)}
              >
                <Pencil aria-hidden="true" />
                <span>{copy.actions.edit}</span>
              </button>
            </li>
          );
        })}
      </ul>
      {tasks.length > 4 && (
        <button
          className="home-deadline-brief__expand focus-visible-control"
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded((current) => !current)}
        >
          <span>
            {expanded
              ? copy.home.collapseDeadlines
              : copy.home.showRemainingDeadlines(hiddenCount)}
          </span>
          <ChevronRight aria-hidden="true" />
        </button>
      )}
    </section>
  );
}

function greetingForHour(hour: number): string {
  if (hour < 12) return copy.home.morningGreeting;
  if (hour < 18) return copy.home.afternoonGreeting;
  return copy.home.eveningGreeting;
}

function nowBriefHeading(period: HomeBriefPeriod): {
  eyebrow: string;
  title: string;
} {
  if (period === "morning") {
    return {
      eyebrow: copy.home.nowBriefMorningEyebrow,
      title: copy.home.nowBriefMorningTitle,
    };
  }
  if (period === "evening") {
    return {
      eyebrow: copy.home.nowBriefEveningEyebrow,
      title: copy.home.nowBriefEveningTitle,
    };
  }
  return {
    eyebrow: copy.home.nowBriefEyebrow,
    title: copy.home.nowBriefTitle,
  };
}

function briefingHeading(
  nextSchedule: ScheduleEntry | undefined,
  scheduleCount: number,
): string {
  if (nextSchedule) return copy.home.briefingWithNext(nextSchedule.title);
  if (scheduleCount) return copy.home.briefingWithSchedule(scheduleCount);
  return copy.home.briefingEmpty;
}

function briefingSummary(scheduleCount: number, taskCount: number): string {
  const parts = [`일정 ${scheduleCount}개`, `할 일 ${taskCount}개`];
  return parts.join(" · ");
}

function scheduleDetail(entry: ScheduleEntry): string {
  const time = `${formatTime(entry.startsAt)} · ${formatTime(entry.endsAt)}`;
  return entry.notes ? `${time} · ${entry.notes}` : time;
}

function relativeScheduleTime(value: string): string {
  const difference = new Date(value).getTime() - Date.now();
  const minutes = Math.round(difference / 60_000);
  if (minutes > 0 && minutes < 60) return `${minutes}분 뒤`;
  return formatTime(value);
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function dueLabel(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  }).format(new Date(value));
}

function taskDueLabel(task: Task): string {
  const state = taskDueState(task);
  return state === "later" ? dueLabel(task.dueAt ?? "") : dueStateLabel(state);
}

function dueStateLabel(state: TaskDueState): string {
  if (state === "overdue") return copy.home.overdue;
  if (state === "today") return copy.home.dueToday;
  if (state === "tomorrow") return copy.home.dueTomorrow;
  return "";
}

function formatDueTime(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function commandStatus(
  job: AgentJob | undefined,
  message: ConversationMessage | undefined,
): { title: string; description: string; needsReview: boolean } | undefined {
  if (!job) return undefined;
  if (job.state === "waiting_approval") {
    return {
      title: copy.home.commandNeedsReview,
      description: copy.home.commandNeedsReviewDescription,
      needsReview: true,
    };
  }
  if (["failed", "cancelled"].includes(job.state) && message?.content.trim()) {
    return {
      title: copy.home.commandResponseReceived,
      description: message.content.trim(),
      needsReview: true,
    };
  }
  if (["failed", "cancelled", "declined"].includes(job.state)) {
    return {
      title: copy.home.commandFailedTitle,
      description: copy.home.commandFailed,
      needsReview: true,
    };
  }
  if (job.state === "completed") {
    return {
      title: copy.home.commandCompleted,
      description: message?.content || copy.home.commandCompletedDescription,
      needsReview: Boolean(message?.content),
    };
  }
  return {
    title: copy.home.commandProcessing,
    description: copy.home.commandProcessingDescription,
    needsReview: false,
  };
}
