import {
  ArrowLeft,
  BriefcaseBusiness,
  CalendarDays,
  ChevronRight,
  Circle,
  CircleAlert,
  RotateCcw,
  Trash2,
  FolderKanban,
  History,
  ListTodo,
  MessageSquareText,
  Pencil,
  PlugZap,
  Plus,
} from "lucide-react";
import { FormEvent, type ReactNode, useEffect, useRef, useState } from "react";

import { type Project, type Workspace } from "../api/projects";
import {
  type GoogleChatAccount,
  type GoogleChatSpace,
  type ProjectGoogleChatSource,
  type ProjectInflowItem,
} from "../api/googleChat";
import { type Goal } from "../api/goals";
import { type Task } from "../api/planning";
import {
  type ManagedWebhookProvider,
  type ProjectWebhook,
  type ProjectWebhookEvent,
  type WebhookMentionDirectory,
  type WebhookDestinationMode,
  type WebhookDelivery,
} from "../api/webhooks";
import { copy } from "../copy";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import { EmptySurface } from "./HomeWorkspace";
import { GoalsPanel } from "./GoalsPanel";
import { ProjectWebhookPanel } from "./ProjectWebhookPanel";
import {
  ProjectInflowPanel,
  type PromoteInflowInput,
} from "./ProjectInflowPanel";

type ProjectDetailTab = "tasks" | "inflow" | "integrations" | "activity";
const PROJECT_DETAIL_TABS: ProjectDetailTab[] = [
  "tasks",
  "inflow",
  "integrations",
  "activity",
];

type ProjectsWorkspaceProps = {
  workspaces: Workspace[];
  goals: Goal[];
  projects: Project[];
  tasks: Task[];
  webhooks: ProjectWebhook[];
  webhookDeliveries: WebhookDelivery[];
  googleChatAccountsAvailable: boolean;
  googleChatAccounts: GoogleChatAccount[];
  googleChatSpaces: GoogleChatSpace[];
  googleChatSources: ProjectGoogleChatSource[];
  projectInflowItems: ProjectInflowItem[];
  selectedWorkspaceId: string | undefined;
  selectedProjectId: string | undefined;
  highlightedTaskId: string | undefined;
  loaded: boolean;
  loading: boolean;
  webhookLoading: boolean;
  inflowLoading: boolean;
  saving: boolean;
  error: string | undefined;
  inflowError: string | undefined;
  onSelectWorkspace(workspaceId: string): void;
  onSelectProject(projectId: string): void;
  onOpenGoalTask(taskId: string, projectId: string): void;
  onClearProject(): void;
  onCreateProject(input: {
    title: string;
    objective?: string;
    managementMode: Project["managementMode"];
    reportingEnabled: boolean;
    staleThresholdDays: number;
    riskLevel: number;
    nextAction?: string;
    dueAt?: string;
  }): Promise<void>;
  onCreateGoal(input: {
    title: string;
    desiredOutcome: string;
    projectId?: string;
    targetAt?: string;
  }): Promise<void>;
  onUpdateGoal(
    goal: Goal,
    input: {
      title: string;
      desiredOutcome: string;
      status: Goal["status"];
      projectId?: string;
      targetAt?: string;
    },
  ): Promise<void>;
  onUpdateProject(
    project: Project,
    input: {
      title: string;
      objective?: string;
      status: Project["status"];
      managementMode: Project["managementMode"];
      reportingEnabled: boolean;
      staleThresholdDays: number;
      riskLevel: number;
      nextAction?: string;
      dueAt?: string;
    },
  ): Promise<void>;
  onDeleteProject(project: Project): Promise<void>;
  onCreateTask(input: { title: string; parentTaskId?: string }): Promise<void>;
  onCompleteTask(task: Task): Promise<void>;
  onUpdateTask(
    task: Task,
    input: {
      title: string;
      notes?: string;
      assigneeName?: string;
      status: Task["status"];
      priority: number;
      dueAt?: string;
      parentTaskId?: string | null;
    },
  ): Promise<void>;
  onDeleteTask(task: Task): Promise<void>;
  onCreateWebhook(input: {
    provider: ManagedWebhookProvider;
    url: string;
    events: ProjectWebhookEvent[];
    mentionDirectory: WebhookMentionDirectory;
  }): Promise<void>;
  onUpdateWebhook(
    webhook: ProjectWebhook,
    input: {
      provider: ManagedWebhookProvider;
      destinationMode: WebhookDestinationMode;
      url?: string;
      events: ProjectWebhookEvent[];
      enabled: boolean;
      mentionDirectory: WebhookMentionDirectory;
    },
  ): Promise<void>;
  onTestWebhook(webhook: ProjectWebhook): Promise<void>;
  onDeleteWebhook(webhook: ProjectWebhook): Promise<void>;
  onRetryWebhookDelivery(delivery: WebhookDelivery): Promise<void>;
  onConnectGoogleChatAccount(): Promise<void>;
  onLoadGoogleChatSpaces(accountId: string): Promise<void>;
  onCreateGoogleChatSource(input: {
    accountId: string;
    spaceName: string;
    displayName: string;
    acknowledgeWithReaction: boolean;
    importHistory: boolean;
  }): Promise<void>;
  onDeleteGoogleChatSource(source: ProjectGoogleChatSource): Promise<void>;
  onSyncGoogleChatSource(source: ProjectGoogleChatSource): Promise<void>;
  onPromoteInflow(
    item: ProjectInflowItem,
    input: PromoteInflowInput,
  ): Promise<void>;
  onDismissInflow(item: ProjectInflowItem): Promise<void>;
  onRetryInflowAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryInflowCompletion(item: ProjectInflowItem): Promise<void>;
};

export function ProjectsWorkspace({
  workspaces,
  goals,
  projects,
  tasks,
  webhooks,
  webhookDeliveries,
  googleChatAccountsAvailable,
  googleChatAccounts,
  googleChatSpaces,
  googleChatSources,
  projectInflowItems,
  selectedWorkspaceId,
  selectedProjectId,
  highlightedTaskId,
  loaded,
  loading,
  webhookLoading,
  inflowLoading,
  saving,
  error,
  inflowError,
  onSelectWorkspace,
  onSelectProject,
  onOpenGoalTask,
  onClearProject,
  onCreateProject,
  onCreateGoal,
  onUpdateGoal,
  onUpdateProject,
  onDeleteProject,
  onCreateTask,
  onCompleteTask,
  onUpdateTask,
  onDeleteTask,
  onCreateWebhook,
  onUpdateWebhook,
  onTestWebhook,
  onDeleteWebhook,
  onRetryWebhookDelivery,
  onConnectGoogleChatAccount,
  onLoadGoogleChatSpaces,
  onCreateGoogleChatSource,
  onDeleteGoogleChatSource,
  onSyncGoogleChatSource,
  onPromoteInflow,
  onDismissInflow,
  onRetryInflowAnalysis,
  onRetryInflowCompletion,
}: ProjectsWorkspaceProps) {
  const [formOpen, setFormOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [objective, setObjective] = useState("");
  const [nextAction, setNextAction] = useState("");
  const [managementMode, setManagementMode] =
    useState<Project["managementMode"]>("completion");
  const [reportingEnabled, setReportingEnabled] = useState(true);
  const [staleThresholdDays, setStaleThresholdDays] = useState("7");
  const [riskLevel, setRiskLevel] = useState("0");
  const [dueDate, setDueDate] = useState("");
  const [taskTitle, setTaskTitle] = useState("");
  const [taskParentId, setTaskParentId] = useState("");
  const [formError, setFormError] = useState<string>();
  const [editingProjectId, setEditingProjectId] = useState<string>();
  const [savedProjectId, setSavedProjectId] = useState<string>();
  const [selectedTaskId, setSelectedTaskId] = useState<string>();
  const [editingTaskId, setEditingTaskId] = useState<string>();
  const [activeProjectTab, setActiveProjectTab] =
    useState<ProjectDetailTab>("tasks");
  const [restoreListFocus, setRestoreListFocus] = useState(false);
  const projectListHeadingRef = useRef<HTMLHeadingElement | null>(null);
  const highlightedTaskRef = useRef<HTMLLIElement | null>(null);
  const initialLoading = loading && !loaded;
  const skeletonVisible = useDelayedSkeleton(initialLoading);
  const showingSkeleton = initialLoading || skeletonVisible;

  const selectedProject = projects.find(
    (project) => project.id === selectedProjectId,
  );
  const openTasks = tasks.filter((task) => task.status === "open");
  const completedTasks = tasks.filter((task) => task.status === "completed");
  const rootTasks = tasks.filter((task) => !task.parentTaskId);
  const rootOpenTasks = openTasks.filter((task) => !task.parentTaskId);
  const openTaskRows = taskHierarchyRows(openTasks);

  useEffect(() => {
    setTaskTitle("");
    setTaskParentId("");
    setEditingProjectId(undefined);
    setSavedProjectId(undefined);
    setSelectedTaskId(undefined);
    setEditingTaskId(undefined);
    setActiveProjectTab("tasks");
  }, [selectedProjectId]);

  useEffect(() => {
    if (selectedProjectId || !restoreListFocus) return;
    projectListHeadingRef.current?.focus();
    setRestoreListFocus(false);
  }, [restoreListFocus, selectedProjectId]);

  useEffect(() => {
    if (!highlightedTaskId) return;
    const element = highlightedTaskRef.current;
    if (!element) return;
    element.scrollIntoView({
      block: "center",
      behavior: preferredScrollBehavior(),
    });
    element.focus({ preventScroll: true });
  }, [highlightedTaskId, tasks]);

  async function submitProject(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim()) {
      setFormError(copy.projects.titleRequired);
      return;
    }
    setFormError(undefined);
    try {
      await onCreateProject({
        title: title.trim(),
        objective: objective.trim() || undefined,
        managementMode,
        reportingEnabled,
        staleThresholdDays: Number(staleThresholdDays),
        riskLevel: Number(riskLevel),
        nextAction: nextAction.trim() || undefined,
        dueAt: dateInputToIso(dueDate),
      });
      setTitle("");
      setObjective("");
      setNextAction("");
      setManagementMode("completion");
      setReportingEnabled(true);
      setStaleThresholdDays("7");
      setRiskLevel("0");
      setDueDate("");
      setFormOpen(false);
    } catch {
      setFormError(copy.projects.projectSaveNotice);
    }
  }

  async function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!taskTitle.trim()) return;
    try {
      await onCreateTask({
        title: taskTitle.trim(),
        parentTaskId: taskParentId || undefined,
      });
      setTaskTitle("");
      setTaskParentId("");
    } catch {
      setFormError(copy.projects.taskSaveNotice);
    }
  }

  return (
    <section
      className="projects-page"
      aria-busy={loading || saving}
      data-refreshing={loading && loaded}
    >
      <header className="projects-heading">
        <div>
          <p>{copy.projects.eyebrow}</p>
          <h1>{copy.projects.title}</h1>
          <span>{copy.projects.description}</span>
        </div>
        <button
          className="primary-button focus-visible-control"
          type="button"
          onClick={() => setFormOpen((open) => !open)}
          aria-expanded={formOpen}
        >
          <Plus aria-hidden="true" />
          {copy.actions.createProject}
        </button>
      </header>

      <div
        className="workspace-tabs"
        role="tablist"
        aria-label={copy.projects.scopeLabel}
      >
        {workspaces.length ? (
          workspaces.map((workspace) => {
            const selected = workspace.id === selectedWorkspaceId;
            return (
              <button
                className="workspace-tabs__button focus-visible-control"
                data-active={selected}
                type="button"
                role="tab"
                aria-selected={selected}
                key={workspace.id}
                onClick={() => onSelectWorkspace(workspace.id)}
              >
                <BriefcaseBusiness aria-hidden="true" />
                {workspace.name}
              </button>
            );
          })
        ) : showingSkeleton ? (
          <WorkspaceTabsSkeleton visible={skeletonVisible} />
        ) : null}
      </div>

      <details className="project-goals-drawer">
        <summary className="focus-visible-control">
          <span>
            <strong>{copy.projects.goalsSummary}</strong>
            <small>{copy.projects.goalsSummaryDescription(goals.length)}</small>
          </span>
          <ChevronRight aria-hidden="true" />
        </summary>
        <GoalsPanel
          goals={goals}
          projects={projects}
          workspaceId={selectedWorkspaceId}
          saving={saving}
          onCreate={onCreateGoal}
          onUpdate={onUpdateGoal}
          onOpenTask={onOpenGoalTask}
          onOpenProject={onSelectProject}
        />
      </details>

      {(error || formError) && (
        <p className="inline-alert" role="alert">
          {formError || error}
        </p>
      )}

      {formOpen && (
        <form
          className="project-create-form"
          onSubmit={(event) => void submitProject(event)}
        >
          <div className="project-create-form__heading">
            <div>
              <h2>{copy.projects.createTitle}</h2>
              <p>{copy.projects.createDescription}</p>
            </div>
          </div>
          <label htmlFor="project-title">
            <span>{copy.projects.projectNameLabel}</span>
            <input
              id="project-title"
              autoFocus
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              disabled={saving}
              maxLength={200}
              placeholder={copy.projects.projectNameHint}
            />
          </label>
          <label htmlFor="project-objective">
            <span>{copy.projects.objectiveLabel}</span>
            <textarea
              id="project-objective"
              value={objective}
              onChange={(event) => setObjective(event.target.value)}
              disabled={saving}
              maxLength={10000}
              placeholder={copy.projects.objectiveHint}
              rows={3}
            />
          </label>
          <label htmlFor="project-next-action">
            <span>{copy.projects.nextActionLabel}</span>
            <input
              id="project-next-action"
              value={nextAction}
              onChange={(event) => setNextAction(event.target.value)}
              disabled={saving}
              maxLength={500}
              placeholder={copy.projects.nextActionHint}
            />
          </label>
          <div className="project-create-form__split">
            <label htmlFor="project-management-mode">
              <span>{copy.projects.managementModeLabel}</span>
              <select
                id="project-management-mode"
                value={managementMode}
                onChange={(event) =>
                  setManagementMode(
                    event.target.value as Project["managementMode"],
                  )
                }
                disabled={saving}
              >
                <option value="completion">
                  {copy.projects.managementModes.completion}
                </option>
                <option value="operation">
                  {copy.projects.managementModes.operation}
                </option>
              </select>
              <small>
                {copy.projects.managementModeDescription[managementMode]}
              </small>
            </label>
            {managementMode === "operation" ? (
              <label htmlFor="project-stale-threshold">
                <span>{copy.projects.staleThresholdLabel}</span>
                <select
                  id="project-stale-threshold"
                  value={staleThresholdDays}
                  onChange={(event) =>
                    setStaleThresholdDays(event.target.value)
                  }
                  disabled={saving}
                >
                  {[3, 5, 7, 14, 30].map((days) => (
                    <option value={days} key={days}>
                      {copy.projects.staleThresholdOption(days)}
                    </option>
                  ))}
                </select>
                <small>{copy.projects.staleThresholdDescription}</small>
              </label>
            ) : (
              <label className="project-reporting-toggle">
                <input
                  type="checkbox"
                  checked={reportingEnabled}
                  onChange={(event) =>
                    setReportingEnabled(event.target.checked)
                  }
                  disabled={saving}
                />
                <span>{copy.projects.weeklyReportingLabel}</span>
                <small>{copy.projects.weeklyReportingDescription}</small>
              </label>
            )}
          </div>
          {managementMode === "operation" && (
            <label className="project-reporting-toggle">
              <input
                type="checkbox"
                checked={reportingEnabled}
                onChange={(event) => setReportingEnabled(event.target.checked)}
                disabled={saving}
              />
              <span>{copy.projects.weeklyReportingLabel}</span>
              <small>{copy.projects.weeklyReportingDescription}</small>
            </label>
          )}
          <div className="project-create-form__split">
            <label htmlFor="project-risk-level">
              <span>{copy.projects.riskLabel}</span>
              <select
                id="project-risk-level"
                value={riskLevel}
                onChange={(event) => setRiskLevel(event.target.value)}
                disabled={saving}
              >
                <option value="0">{copy.projects.riskLevels[0]}</option>
                <option value="1">{copy.projects.riskLevels[1]}</option>
                <option value="2">{copy.projects.riskLevels[2]}</option>
                <option value="3">{copy.projects.riskLevels[3]}</option>
              </select>
            </label>
            <label htmlFor="project-due-date">
              <span>{copy.projects.dueDateLabel}</span>
              <input
                id="project-due-date"
                type="date"
                value={dueDate}
                onInput={(event) => setDueDate(event.currentTarget.value)}
                disabled={saving}
              />
            </label>
          </div>
          <div className="project-create-form__actions">
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => setFormOpen(false)}
            >
              {copy.actions.cancel}
            </button>
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={saving}
            >
              {saving ? copy.actions.saving : copy.actions.createProject}
            </button>
          </div>
        </form>
      )}

      <div
        className="projects-layout"
        data-project-selected={Boolean(selectedProject)}
        data-empty={!showingSkeleton && projects.length === 0}
      >
        {!showingSkeleton && projects.length === 0 && !formOpen && (
          <section
            className="projects-zero-state"
            aria-labelledby="projects-zero-state-title"
          >
            <span className="projects-zero-state__icon" aria-hidden="true">
              <FolderKanban />
            </span>
            <div>
              <h2 id="projects-zero-state-title">{copy.projects.emptyTitle}</h2>
              <p>{copy.projects.emptyDescription}</p>
            </div>
            <button
              className="primary-button focus-visible-control"
              type="button"
              onClick={() => setFormOpen(true)}
            >
              <Plus aria-hidden="true" />
              {copy.actions.createProject}
            </button>
          </section>
        )}
        <section
          className="projects-list"
          aria-labelledby="projects-list-title"
        >
          <div className="projects-section-heading">
            <div>
              <FolderKanban aria-hidden="true" />
              <h2
                id="projects-list-title"
                ref={projectListHeadingRef}
                tabIndex={-1}
              >
                {copy.projects.listTitle}
              </h2>
            </div>
            {!showingSkeleton && (
              <span>{copy.projects.projectCount(projects.length)}</span>
            )}
          </div>
          <div className="projects-surface">
            {showingSkeleton ? (
              <ProjectListSkeleton rows={4} visible={skeletonVisible} />
            ) : projects.length ? (
              <ul className="project-list">
                {projects.map((project) => (
                  <li key={project.id}>
                    <button
                      className="project-list__item focus-visible-control"
                      data-active={project.id === selectedProjectId}
                      type="button"
                      onClick={() => {
                        setEditingProjectId(undefined);
                        setSavedProjectId(undefined);
                        onSelectProject(project.id);
                      }}
                    >
                      <span className="project-list__main">
                        <strong>{project.title}</strong>
                      </span>
                      <span className="project-list__meta">
                        <span>
                          {project.managementMode === "completion"
                            ? copy.projects.projectProgress(
                                project.progressPercent,
                              )
                            : copy.projects.operationSummary(
                                project.openTaskCount,
                                project.backlogDelta,
                              )}
                        </span>
                        <small>
                          {copy.projects.openTaskCount(project.openTaskCount)}
                        </small>
                        <ChevronRight aria-hidden="true" />
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <EmptySurface
                title={copy.projects.emptyTitle}
                description={copy.projects.emptyDescription}
              />
            )}
          </div>
        </section>

        <section
          className="project-detail"
          aria-labelledby="project-detail-title"
        >
          {showingSkeleton ? (
            <div className="project-detail__panel">
              <ProjectDetailSkeleton visible={skeletonVisible} />
            </div>
          ) : selectedProject ? (
            <>
              <button
                className="project-detail__back focus-visible-control"
                type="button"
                onClick={() => {
                  setRestoreListFocus(true);
                  onClearProject();
                }}
              >
                <ArrowLeft aria-hidden="true" />
                {copy.projects.backToList}
              </button>
              <nav
                className="project-detail-tabs"
                role="tablist"
                aria-label={copy.projects.detailTabsLabel}
              >
                <ProjectDetailTabButton
                  id="tasks"
                  active={activeProjectTab === "tasks"}
                  label={copy.projects.detailTabs.tasks}
                  count={openTasks.length}
                  icon={<ListTodo aria-hidden="true" />}
                  onSelect={setActiveProjectTab}
                />
                <ProjectDetailTabButton
                  id="inflow"
                  active={activeProjectTab === "inflow"}
                  label={copy.projects.detailTabs.inflow}
                  count={projectInflowItems.length}
                  icon={<MessageSquareText aria-hidden="true" />}
                  onSelect={setActiveProjectTab}
                />
                <ProjectDetailTabButton
                  id="integrations"
                  active={activeProjectTab === "integrations"}
                  label={copy.projects.detailTabs.integrations}
                  count={webhooks.length}
                  icon={<PlugZap aria-hidden="true" />}
                  onSelect={setActiveProjectTab}
                />
                <ProjectDetailTabButton
                  id="activity"
                  active={activeProjectTab === "activity"}
                  label={copy.projects.detailTabs.activity}
                  count={completedTasks.length}
                  icon={<History aria-hidden="true" />}
                  onSelect={setActiveProjectTab}
                />
              </nav>
              <section className="project-detail__panel project-detail__overview">
                <div className="project-detail__heading">
                  <div>
                    <p>{copy.projects.projectDetailLabel}</p>
                    <h2 id="project-detail-title">{selectedProject.title}</h2>
                    <span>
                      {selectedProject.objective ||
                        copy.projects.objectiveEmpty}
                    </span>
                  </div>
                  <div className="project-detail__heading-actions">
                    {selectedProject.riskLevel > 0 && (
                      <span
                        className="project-risk"
                        data-risk={selectedProject.riskLevel}
                      >
                        <CircleAlert aria-hidden="true" />
                        {riskLabel(selectedProject.riskLevel)}
                      </span>
                    )}
                    <button
                      className="secondary-button focus-visible-control"
                      type="button"
                      disabled={saving}
                      onClick={() => {
                        setFormError(undefined);
                        setSavedProjectId(undefined);
                        setEditingProjectId(selectedProject.id);
                      }}
                    >
                      <Pencil aria-hidden="true" />
                      {copy.projects.editProject}
                    </button>
                  </div>
                </div>
                <div
                  className="project-detail__meta"
                  aria-label={copy.projects.currentStateLabel}
                >
                  <span>
                    <strong>{copy.projects.statusLabel}</strong>
                    {statusLabel(selectedProject.status)}
                  </span>
                  <span>
                    <strong>{copy.projects.managementModeLabel}</strong>
                    {
                      copy.projects.managementModes[
                        selectedProject.managementMode
                      ]
                    }
                  </span>
                  <span>
                    <CalendarDays aria-hidden="true" />
                    <strong>{copy.projects.dueDateLabel}</strong>
                    {formatDueDate(selectedProject.dueAt)}
                  </span>
                </div>
                {selectedProject.managementMode === "completion" ? (
                  <CompletionProjectProgress project={selectedProject} />
                ) : (
                  <OperationProjectHealth project={selectedProject} />
                )}
                {editingProjectId === selectedProject.id ? (
                  <ProjectEditForm
                    key={`${selectedProject.id}:${selectedProject.version}`}
                    project={selectedProject}
                    saving={saving}
                    onCancel={() => {
                      setFormError(undefined);
                      setEditingProjectId(undefined);
                    }}
                    onSave={async (input) => {
                      setFormError(undefined);
                      try {
                        await onUpdateProject(selectedProject, input);
                        setEditingProjectId(undefined);
                        setSavedProjectId(selectedProject.id);
                      } catch {
                        setFormError(copy.projects.projectUpdateNotice);
                      }
                    }}
                    onDelete={async () => {
                      await onDeleteProject(selectedProject);
                      setRestoreListFocus(true);
                    }}
                  />
                ) : (
                  <div className="project-next-action">
                    <span>{copy.projects.nextActionLabel}</span>
                    <strong>
                      {selectedProject.nextAction || copy.projects.noNextAction}
                    </strong>
                  </div>
                )}
                {savedProjectId === selectedProject.id && (
                  <p className="project-save-status" role="status">
                    {copy.projects.projectUpdated}
                  </p>
                )}
              </section>
              {activeProjectTab === "tasks" && (
                <div
                  className="project-detail__tasks"
                  role="tabpanel"
                  aria-label={copy.projects.detailTabs.tasks}
                >
                  <div className="projects-section-heading">
                    <div>
                      <ListTodo aria-hidden="true" />
                      <h3>{copy.projects.workItemsTitle}</h3>
                    </div>
                    <span>{copy.projects.openTaskCount(openTasks.length)}</span>
                  </div>
                  {openTasks.length ? (
                    <ul className="project-task-list">
                      {openTaskRows.map(({ task, depth, childCount }) => (
                        <li
                          key={task.id}
                          data-task-depth={depth}
                          ref={
                            highlightedTaskId === task.id
                              ? highlightedTaskRef
                              : undefined
                          }
                          data-highlighted={highlightedTaskId === task.id}
                          tabIndex={
                            highlightedTaskId === task.id ? -1 : undefined
                          }
                        >
                          <button
                            className="project-task-list__complete focus-visible-control"
                            type="button"
                            disabled={saving || childCount > 0}
                            aria-label={copy.home.completeTask(task.title)}
                            title={
                              childCount > 0
                                ? copy.projects.completeChildrenFirst
                                : undefined
                            }
                            onClick={() => void onCompleteTask(task)}
                          >
                            <Circle aria-hidden="true" />
                          </button>
                          <button
                            className="project-task-list__content focus-visible-control"
                            type="button"
                            aria-expanded={selectedTaskId === task.id}
                            onClick={() =>
                              setSelectedTaskId((current) =>
                                current === task.id ? undefined : task.id,
                              )
                            }
                          >
                            <span className="project-task-list__details">
                              <strong>{task.title}</strong>
                              <span>
                                {taskMeta(task)}
                                {childCount > 0 &&
                                  ` · ${copy.projects.subtaskCount(childCount)}`}
                              </span>
                            </span>
                            <span
                              className="project-task-list__assignee"
                              data-assigned={Boolean(task.assigneeName)}
                            >
                              {copy.projects.taskAssignee(
                                task.assigneeName ?? undefined,
                              )}
                            </span>
                          </button>
                          <button
                            className="project-task-list__edit focus-visible-control"
                            type="button"
                            aria-label={copy.projects.editWorkItem(task.title)}
                            onClick={() => {
                              setSelectedTaskId(task.id);
                              setEditingTaskId(task.id);
                            }}
                          >
                            <Pencil aria-hidden="true" />
                          </button>
                          {selectedTaskId === task.id &&
                            editingTaskId !== task.id && (
                              <TaskDetail
                                task={task}
                                parentTask={tasks.find(
                                  (item) => item.id === task.parentTaskId,
                                )}
                              />
                            )}
                          {editingTaskId === task.id && (
                            <TaskEditForm
                              task={task}
                              parentCandidates={rootOpenTasks.filter(
                                (item) => item.id !== task.id,
                              )}
                              saving={saving}
                              onCancel={() => setEditingTaskId(undefined)}
                              onSave={async (input) => {
                                await onUpdateTask(task, input);
                                setEditingTaskId(undefined);
                              }}
                              onDelete={() => onDeleteTask(task)}
                            />
                          )}
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="project-detail__empty">
                      {copy.projects.workItemsEmpty}
                    </p>
                  )}
                  <form
                    className="project-task-form"
                    onSubmit={(event) => void submitTask(event)}
                  >
                    <label className="sr-only" htmlFor="project-task-title">
                      {copy.projects.workItemLabel}
                    </label>
                    <input
                      id="project-task-title"
                      value={taskTitle}
                      onChange={(event) => setTaskTitle(event.target.value)}
                      disabled={
                        saving || selectedProject.status === "completed"
                      }
                      maxLength={200}
                      placeholder={copy.projects.workItemHint}
                    />
                    <label className="sr-only" htmlFor="project-task-parent">
                      {copy.projects.parentTaskLabel}
                    </label>
                    <select
                      id="project-task-parent"
                      value={taskParentId}
                      disabled={
                        saving || selectedProject.status === "completed"
                      }
                      onChange={(event) => setTaskParentId(event.target.value)}
                    >
                      <option value="">{copy.projects.parentTaskNone}</option>
                      {rootOpenTasks.map((task) => (
                        <option value={task.id} key={task.id}>
                          {task.title}
                        </option>
                      ))}
                    </select>
                    <button
                      className="secondary-button focus-visible-control"
                      type="submit"
                      disabled={
                        saving ||
                        selectedProject.status === "completed" ||
                        !taskTitle.trim()
                      }
                    >
                      {copy.actions.addWorkItem}
                    </button>
                  </form>
                  {selectedProject.status === "completed" && (
                    <p className="project-detail__empty">
                      {copy.projects.completedProjectNotice}
                    </p>
                  )}
                </div>
              )}
              {activeProjectTab === "activity" && (
                <div
                  className="project-detail__tasks"
                  role="tabpanel"
                  aria-label={copy.projects.detailTabs.activity}
                >
                  {completedTasks.length > 0 && (
                    <section
                      className="project-completed-tasks"
                      aria-labelledby="completed-tasks-title"
                    >
                      <div className="projects-section-heading">
                        <div>
                          <Circle aria-hidden="true" />
                          <h3 id="completed-tasks-title">
                            {copy.projects.completedWorkItemsTitle}
                          </h3>
                        </div>
                        <span>
                          {copy.projects.completedTaskCount(
                            completedTasks.length,
                          )}
                        </span>
                      </div>
                      <ul className="project-task-list project-task-list--completed">
                        {completedTasks.map((task) => (
                          <li key={task.id}>
                            <button
                              className="project-task-list__complete focus-visible-control"
                              type="button"
                              disabled={saving}
                              aria-label={copy.projects.reopenTask(task.title)}
                              onClick={() =>
                                void onUpdateTask(task, {
                                  title: task.title,
                                  notes: task.notes ?? undefined,
                                  assigneeName: task.assigneeName ?? undefined,
                                  status: "open",
                                  priority: task.priority,
                                  dueAt: task.dueAt ?? undefined,
                                })
                              }
                            >
                              <RotateCcw aria-hidden="true" />
                            </button>
                            <button
                              className="project-task-list__content focus-visible-control"
                              type="button"
                              aria-expanded={selectedTaskId === task.id}
                              onClick={() =>
                                setSelectedTaskId((current) =>
                                  current === task.id ? undefined : task.id,
                                )
                              }
                            >
                              <span className="project-task-list__details">
                                <strong>{task.title}</strong>
                                <span>
                                  {copy.projects.completedTaskMeta(
                                    taskMeta(task),
                                  )}
                                </span>
                              </span>
                              <span
                                className="project-task-list__assignee"
                                data-assigned={Boolean(task.assigneeName)}
                              >
                                {copy.projects.taskAssignee(
                                  task.assigneeName ?? undefined,
                                )}
                              </span>
                            </button>
                            <button
                              className="project-task-list__edit focus-visible-control"
                              type="button"
                              aria-label={copy.projects.editWorkItem(
                                task.title,
                              )}
                              onClick={() => {
                                setSelectedTaskId(task.id);
                                setEditingTaskId(task.id);
                              }}
                            >
                              <Pencil aria-hidden="true" />
                            </button>
                            {selectedTaskId === task.id &&
                              editingTaskId !== task.id && (
                                <TaskDetail task={task} />
                              )}
                            {editingTaskId === task.id && (
                              <TaskEditForm
                                task={task}
                                parentCandidates={rootTasks.filter(
                                  (item) => item.id !== task.id,
                                )}
                                saving={saving}
                                onCancel={() => setEditingTaskId(undefined)}
                                onSave={async (input) => {
                                  await onUpdateTask(task, input);
                                  setEditingTaskId(undefined);
                                }}
                                onDelete={() => onDeleteTask(task)}
                              />
                            )}
                          </li>
                        ))}
                      </ul>
                    </section>
                  )}
                  {completedTasks.length === 0 && (
                    <p className="project-detail__empty">
                      {copy.projects.completedWorkItemsEmpty}
                    </p>
                  )}
                </div>
              )}
              {activeProjectTab === "inflow" && (
                <ProjectInflowPanel
                  accountsAvailable={googleChatAccountsAvailable}
                  accounts={googleChatAccounts}
                  spaces={googleChatSpaces}
                  sources={googleChatSources}
                  items={projectInflowItems}
                  loading={inflowLoading}
                  saving={saving}
                  problemMessage={inflowError}
                  onConnectAccount={onConnectGoogleChatAccount}
                  onLoadSpaces={onLoadGoogleChatSpaces}
                  onCreateSource={onCreateGoogleChatSource}
                  onDeleteSource={onDeleteGoogleChatSource}
                  onSyncSource={onSyncGoogleChatSource}
                  onPromote={onPromoteInflow}
                  onDismiss={onDismissInflow}
                  onRetryAnalysis={onRetryInflowAnalysis}
                  onRetryCompletion={onRetryInflowCompletion}
                />
              )}
              {activeProjectTab === "integrations" && (
                <ProjectWebhookPanel
                  projectId={selectedProject.id}
                  webhooks={webhooks}
                  deliveries={webhookDeliveries}
                  loading={webhookLoading}
                  saving={saving}
                  onCreate={onCreateWebhook}
                  onUpdate={onUpdateWebhook}
                  onTest={onTestWebhook}
                  onDelete={onDeleteWebhook}
                  onRetry={onRetryWebhookDelivery}
                />
              )}
            </>
          ) : (
            <div className="project-detail__panel project-detail__selection">
              <EmptySurface
                title={copy.projects.selectTitle}
                description={copy.projects.selectDescription}
              />
            </div>
          )}
        </section>
      </div>
    </section>
  );
}

function ProjectDetailTabButton({
  id,
  active,
  label,
  count,
  icon,
  onSelect,
}: {
  id: ProjectDetailTab;
  active: boolean;
  label: string;
  count: number;
  icon: ReactNode;
  onSelect(tab: ProjectDetailTab): void;
}) {
  return (
    <button
      className="project-detail-tabs__button focus-visible-control"
      data-active={active}
      type="button"
      role="tab"
      aria-selected={active}
      tabIndex={active ? 0 : -1}
      onClick={() => onSelect(id)}
      onKeyDown={(event) => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
          return;
        }
        event.preventDefault();
        const currentIndex = PROJECT_DETAIL_TABS.indexOf(id);
        const nextIndex =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? PROJECT_DETAIL_TABS.length - 1
              : (currentIndex +
                  (event.key === "ArrowRight" ? 1 : -1) +
                  PROJECT_DETAIL_TABS.length) %
                PROJECT_DETAIL_TABS.length;
        const nextTab = PROJECT_DETAIL_TABS[nextIndex];
        onSelect(nextTab);
        const buttons =
          event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
            '[role="tab"]',
          );
        buttons?.[nextIndex]?.focus();
      }}
    >
      {icon}
      <span>{label}</span>
      <small>{count}</small>
    </button>
  );
}

export function taskHierarchyRows(
  tasks: Task[],
): Array<{ task: Task; depth: 0 | 1; childCount: number }> {
  const taskIds = new Set(tasks.map((task) => task.id));
  const roots = tasks.filter(
    (task) => !task.parentTaskId || !taskIds.has(task.parentTaskId),
  );
  const childrenByParent = new Map<string, Task[]>();
  for (const task of tasks) {
    if (!task.parentTaskId || !taskIds.has(task.parentTaskId)) continue;
    const children = childrenByParent.get(task.parentTaskId) ?? [];
    children.push(task);
    childrenByParent.set(task.parentTaskId, children);
  }
  return roots.flatMap((task) => {
    const children = childrenByParent.get(task.id) ?? [];
    return [
      { task, depth: 0 as const, childCount: children.length },
      ...children.map((child) => ({
        task: child,
        depth: 1 as const,
        childCount: 0,
      })),
    ];
  });
}

function CompletionProjectProgress({ project }: { project: Project }) {
  return (
    <section
      className="project-progress"
      aria-labelledby="project-progress-title"
      data-health={project.health}
    >
      <div className="project-progress__heading">
        <div>
          <span id="project-progress-title">{copy.projects.progressTitle}</span>
          <strong>{copy.projects.projectHealth[project.health]}</strong>
        </div>
        <strong>
          {copy.projects.progressPercent(project.progressPercent)}
        </strong>
      </div>
      <div
        className="project-progress__track"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={project.progressPercent}
      >
        <span
          style={{
            width: `${Math.max(0, Math.min(100, project.progressPercent))}%`,
          }}
        />
      </div>
      <div className="project-progress__facts">
        <span>
          {copy.projects.progressSummary(
            project.completedTaskCount,
            project.totalTaskCount,
          )}
        </span>
        {project.overdueTaskCount > 0 && (
          <span data-attention="true">
            {copy.projects.overdueTaskCount(project.overdueTaskCount)}
          </span>
        )}
        {project.unassignedTaskCount > 0 && (
          <span>
            {copy.projects.unassignedTaskCount(project.unassignedTaskCount)}
          </span>
        )}
      </div>
    </section>
  );
}

function OperationProjectHealth({ project }: { project: Project }) {
  const metrics = [
    {
      label: copy.projects.operationMetrics.open,
      value: `${project.openTaskCount}`,
      attention: false,
    },
    {
      label: copy.projects.operationMetrics.inflow,
      value: `${project.weeklyCreatedTaskCount}`,
      attention: false,
    },
    {
      label: copy.projects.operationMetrics.completed,
      value: `${project.weeklyCompletedTaskCount}`,
      attention: false,
    },
    {
      label: copy.projects.operationMetrics.backlog,
      value: copy.projects.backlogDelta(project.backlogDelta),
      attention: project.backlogDelta > 0,
    },
    {
      label: copy.projects.operationMetrics.overdue,
      value: `${project.overdueTaskCount}`,
      attention: project.overdueTaskCount > 0,
    },
    {
      label: copy.projects.operationMetrics.stale,
      value: `${project.staleTaskCount}`,
      attention: project.staleTaskCount > 0,
    },
    {
      label: copy.projects.operationMetrics.cycleTime,
      value: copy.projects.cycleTime(project.averageCycleTimeHours),
      attention: false,
    },
    {
      label: copy.projects.operationMetrics.onTime,
      value: copy.projects.onTimeCompletion(project.onTimeCompletionPercent),
      attention: (project.onTimeCompletionPercent ?? 100) < 80,
    },
  ];
  return (
    <section
      className="project-operation-health"
      aria-labelledby="project-operation-health-title"
      data-health={project.health}
    >
      <header>
        <div>
          <span id="project-operation-health-title">
            {copy.projects.operationHealthTitle}
          </span>
          <strong>{copy.projects.projectHealth[project.health]}</strong>
        </div>
        <small>{copy.projects.operationPeriod}</small>
      </header>
      <dl>
        {metrics.map((metric) => (
          <div key={metric.label} data-attention={metric.attention}>
            <dt>{metric.label}</dt>
            <dd>{metric.value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function ProjectEditForm({
  project,
  saving,
  onCancel,
  onSave,
  onDelete,
}: {
  project: Project;
  saving: boolean;
  onCancel(): void;
  onSave(input: {
    title: string;
    objective?: string;
    status: Project["status"];
    managementMode: Project["managementMode"];
    reportingEnabled: boolean;
    staleThresholdDays: number;
    riskLevel: number;
    nextAction?: string;
    dueAt?: string;
  }): Promise<void>;
  onDelete(): Promise<void>;
}) {
  const [title, setTitle] = useState(project.title);
  const [objective, setObjective] = useState(project.objective ?? "");
  const [status, setStatus] = useState<Project["status"]>(project.status);
  const [managementMode, setManagementMode] = useState<
    Project["managementMode"]
  >(project.managementMode);
  const [reportingEnabled, setReportingEnabled] = useState(
    project.reportingEnabled,
  );
  const [staleThresholdDays, setStaleThresholdDays] = useState(
    String(project.staleThresholdDays),
  );
  const [riskLevel, setRiskLevel] = useState(String(project.riskLevel));
  const [nextAction, setNextAction] = useState(project.nextAction ?? "");
  const [dueDate, setDueDate] = useState(isoToDateInput(project.dueAt));
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string>();
  const deleteTriggerRef = useRef<HTMLButtonElement>(null);
  const deleteSafeActionRef = useRef<HTMLButtonElement>(null);
  const restoreDeleteTriggerRef = useRef(false);
  const busy = saving || deleting;

  useEffect(() => {
    const target = confirmingDelete
      ? deleteSafeActionRef.current
      : restoreDeleteTriggerRef.current
        ? deleteTriggerRef.current
        : undefined;
    if (!target) return;
    const frame = window.requestAnimationFrame(() => {
      target.focus();
      if (!confirmingDelete) restoreDeleteTriggerRef.current = false;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [confirmingDelete]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    if (!title.trim()) {
      setError(copy.projects.titleRequired);
      return;
    }
    setError(undefined);
    await onSave({
      title: title.trim(),
      objective: objective.trim() || undefined,
      status,
      managementMode,
      reportingEnabled,
      staleThresholdDays: Number(staleThresholdDays),
      riskLevel: Number(riskLevel),
      nextAction: nextAction.trim() || undefined,
      dueAt: dateInputToIso(dueDate),
    });
  }

  async function removeProject() {
    if (saving || deleting) return;
    setDeleting(true);
    setError(undefined);
    try {
      await onDelete();
    } catch {
      setError(copy.projects.projectDeleteNotice);
      restoreDeleteTriggerRef.current = true;
      setConfirmingDelete(false);
      setDeleting(false);
    }
  }

  return (
    <form
      className="project-edit-form"
      aria-labelledby="project-edit-title"
      aria-busy={busy}
      onSubmit={(event) => void submit(event)}
    >
      <div className="project-edit-form__heading">
        <div>
          <h3 id="project-edit-title">{copy.projects.editTitle}</h3>
          <p>{copy.projects.editDescription}</p>
        </div>
      </div>
      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}
      <label htmlFor="project-edit-name">
        <span>{copy.projects.projectNameLabel}</span>
        <input
          id="project-edit-name"
          value={title}
          maxLength={200}
          disabled={busy}
          onChange={(event) => setTitle(event.target.value)}
        />
      </label>
      <label htmlFor="project-edit-objective">
        <span>{copy.projects.objectiveLabel}</span>
        <textarea
          id="project-edit-objective"
          value={objective}
          maxLength={10_000}
          rows={3}
          disabled={busy}
          onChange={(event) => setObjective(event.target.value)}
        />
      </label>
      <label htmlFor="project-edit-next-action">
        <span>{copy.projects.nextActionLabel}</span>
        <input
          id="project-edit-next-action"
          value={nextAction}
          maxLength={500}
          disabled={busy}
          onChange={(event) => setNextAction(event.target.value)}
        />
      </label>
      <div className="project-edit-form__fields">
        <label htmlFor="project-edit-management-mode">
          <span>{copy.projects.managementModeLabel}</span>
          <select
            id="project-edit-management-mode"
            value={managementMode}
            disabled={busy}
            onChange={(event) =>
              setManagementMode(event.target.value as Project["managementMode"])
            }
          >
            <option value="completion">
              {copy.projects.managementModes.completion}
            </option>
            <option value="operation">
              {copy.projects.managementModes.operation}
            </option>
          </select>
        </label>
        {managementMode === "operation" && (
          <label htmlFor="project-edit-stale-threshold">
            <span>{copy.projects.staleThresholdLabel}</span>
            <select
              id="project-edit-stale-threshold"
              value={staleThresholdDays}
              disabled={busy}
              onChange={(event) => setStaleThresholdDays(event.target.value)}
            >
              {[3, 5, 7, 14, 30].map((days) => (
                <option value={days} key={days}>
                  {copy.projects.staleThresholdOption(days)}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="project-reporting-toggle">
          <input
            type="checkbox"
            checked={reportingEnabled}
            disabled={busy}
            onChange={(event) => setReportingEnabled(event.target.checked)}
          />
          <span>{copy.projects.weeklyReportingLabel}</span>
          <small>{copy.projects.weeklyReportingDescription}</small>
        </label>
      </div>
      <div className="project-edit-form__fields">
        <label htmlFor="project-edit-status">
          <span>{copy.projects.statusLabel}</span>
          <select
            id="project-edit-status"
            value={status}
            disabled={busy}
            onChange={(event) =>
              setStatus(event.target.value as Project["status"])
            }
          >
            <option value="active">{copy.projects.statuses.active}</option>
            <option value="paused">{copy.projects.statuses.paused}</option>
            <option value="completed">
              {copy.projects.statuses.completed}
            </option>
          </select>
        </label>
        <label htmlFor="project-edit-risk">
          <span>{copy.projects.riskLabel}</span>
          <select
            id="project-edit-risk"
            value={riskLevel}
            disabled={busy}
            onChange={(event) => setRiskLevel(event.target.value)}
          >
            {copy.projects.riskLevels.map((label, level) => (
              <option key={label} value={level}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label htmlFor="project-edit-due-date">
          <span>{copy.projects.dueDateLabel}</span>
          <input
            id="project-edit-due-date"
            type="date"
            value={dueDate}
            disabled={busy}
            onInput={(event) => setDueDate(event.currentTarget.value)}
          />
        </label>
      </div>
      {confirmingDelete ? (
        <section
          className="project-edit-form__delete-confirmation"
          role="group"
          aria-label={copy.projects.deleteProjectTitle}
        >
          <div>
            <strong>{copy.projects.deleteProjectTitle}</strong>
            <p>{copy.projects.deleteProjectDescription}</p>
          </div>
          <div>
            <button
              ref={deleteSafeActionRef}
              className="secondary-button focus-visible-control"
              type="button"
              disabled={busy}
              onClick={() => {
                restoreDeleteTriggerRef.current = true;
                setConfirmingDelete(false);
              }}
            >
              {copy.projects.keepProject}
            </button>
            <button
              className="destructive-button focus-visible-control"
              type="button"
              disabled={busy}
              onClick={() => void removeProject()}
            >
              {deleting ? (
                <span className="button-spinner" aria-hidden="true" />
              ) : (
                <Trash2 aria-hidden="true" />
              )}
              {deleting ? copy.actions.deleting : copy.projects.deleteProject}
            </button>
          </div>
        </section>
      ) : (
        <div className="project-edit-form__actions">
          <button
            ref={deleteTriggerRef}
            className="destructive-quiet-button focus-visible-control"
            type="button"
            disabled={busy}
            onClick={() => setConfirmingDelete(true)}
          >
            <Trash2 aria-hidden="true" />
            {copy.projects.deleteProject}
          </button>
          <span />
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={busy}
            onClick={onCancel}
          >
            {copy.projects.stopEditing}
          </button>
          <button
            className="primary-button focus-visible-control"
            type="submit"
            disabled={busy}
          >
            {saving ? copy.actions.saving : copy.projects.saveChanges}
          </button>
        </div>
      )}
    </form>
  );
}

function TaskDetail({ task, parentTask }: { task: Task; parentTask?: Task }) {
  return (
    <section
      className="project-task-detail"
      aria-label={copy.projects.workItemDetail(task.title)}
    >
      <div>
        <span>{copy.projects.workItemNotesLabel}</span>
        <p>{task.notes || copy.projects.workItemNotesEmpty}</p>
      </div>
      <dl>
        {parentTask && (
          <div>
            <dt>{copy.projects.parentTaskLabel}</dt>
            <dd>{parentTask.title}</dd>
          </div>
        )}
        <div>
          <dt>{copy.projects.workItemAssigneeLabel}</dt>
          <dd>{task.assigneeName || copy.projects.workItemAssigneeEmpty}</dd>
        </div>
        <div>
          <dt>{copy.projects.workItemPriorityLabel}</dt>
          <dd>
            {copy.projects.taskPriorities[task.priority] ??
              copy.projects.taskPriorities[1]}
          </dd>
        </div>
        <div>
          <dt>{copy.projects.dueDateLabel}</dt>
          <dd>
            {task.dueAt
              ? new Date(task.dueAt).toLocaleString("ko-KR")
              : copy.projects.workItemDueEmpty}
          </dd>
        </div>
      </dl>
    </section>
  );
}

function TaskEditForm({
  task,
  parentCandidates,
  saving,
  onCancel,
  onSave,
  onDelete,
}: {
  task: Task;
  parentCandidates: Task[];
  saving: boolean;
  onCancel(): void;
  onSave(input: {
    title: string;
    notes?: string;
    assigneeName?: string;
    status: Task["status"];
    priority: number;
    dueAt?: string;
    parentTaskId?: string | null;
  }): Promise<void>;
  onDelete(): Promise<void>;
}) {
  const [title, setTitle] = useState(task.title);
  const [notes, setNotes] = useState(task.notes ?? "");
  const [assigneeName, setAssigneeName] = useState(task.assigneeName ?? "");
  const [priority, setPriority] = useState(String(task.priority));
  const [dueDate, setDueDate] = useState(isoToDateInput(task.dueAt));
  const [parentTaskId, setParentTaskId] = useState(task.parentTaskId ?? "");
  const [confirmingRemoval, setConfirmingRemoval] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string>();
  const removeTriggerRef = useRef<HTMLButtonElement>(null);
  const removeSafeActionRef = useRef<HTMLButtonElement>(null);
  const restoreRemoveTriggerRef = useRef(false);
  const busy = saving || deleting;

  useEffect(() => {
    const target = confirmingRemoval
      ? removeSafeActionRef.current
      : restoreRemoveTriggerRef.current
        ? removeTriggerRef.current
        : undefined;
    if (!target) return;
    const frame = window.requestAnimationFrame(() => {
      target.focus();
      if (!confirmingRemoval) restoreRemoveTriggerRef.current = false;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [confirmingRemoval]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    if (!title.trim()) {
      setError(copy.projects.workItemTitleRequired);
      return;
    }
    setError(undefined);
    try {
      await onSave({
        title: title.trim(),
        notes: notes.trim() || undefined,
        assigneeName: assigneeName.trim() || undefined,
        status: task.status,
        priority: Number(priority),
        dueAt: dateInputToIso(dueDate),
        parentTaskId: parentTaskId || null,
      });
    } catch {
      setError(copy.projects.taskUpdateNotice);
    }
  }

  async function removeTask() {
    if (busy) return;
    setDeleting(true);
    setError(undefined);
    try {
      await onDelete();
    } catch {
      setError(copy.projects.taskRemoveNotice);
      restoreRemoveTriggerRef.current = true;
      setConfirmingRemoval(false);
      setDeleting(false);
    }
  }

  return (
    <form
      className="project-task-edit-form"
      aria-label={copy.projects.editWorkItem(task.title)}
      aria-busy={busy}
      onSubmit={(event) => void save(event)}
    >
      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}
      <label htmlFor={`task-title-${task.id}`}>
        <span>{copy.projects.workItemTitleLabel}</span>
        <input
          id={`task-title-${task.id}`}
          value={title}
          maxLength={200}
          disabled={busy}
          onChange={(event) => setTitle(event.target.value)}
        />
      </label>
      <label htmlFor={`task-notes-${task.id}`}>
        <span>{copy.projects.workItemNotesLabel}</span>
        <textarea
          id={`task-notes-${task.id}`}
          value={notes}
          maxLength={10_000}
          rows={3}
          disabled={busy}
          placeholder={copy.projects.workItemNotesHint}
          onChange={(event) => setNotes(event.target.value)}
        />
      </label>
      <div className="project-task-edit-form__fields">
        <label htmlFor={`task-parent-${task.id}`}>
          <span>{copy.projects.parentTaskLabel}</span>
          <select
            id={`task-parent-${task.id}`}
            value={parentTaskId}
            disabled={busy}
            onChange={(event) => setParentTaskId(event.target.value)}
          >
            <option value="">{copy.projects.parentTaskNone}</option>
            {parentCandidates.map((candidate) => (
              <option value={candidate.id} key={candidate.id}>
                {candidate.title}
              </option>
            ))}
          </select>
        </label>
        <label htmlFor={`task-assignee-${task.id}`}>
          <span>{copy.projects.workItemAssigneeLabel}</span>
          <input
            id={`task-assignee-${task.id}`}
            value={assigneeName}
            maxLength={80}
            disabled={busy}
            placeholder={copy.projects.workItemAssigneeHint}
            onChange={(event) => setAssigneeName(event.target.value)}
          />
        </label>
        <label htmlFor={`task-priority-${task.id}`}>
          <span>{copy.projects.workItemPriorityLabel}</span>
          <select
            id={`task-priority-${task.id}`}
            value={priority}
            disabled={busy}
            onChange={(event) => setPriority(event.target.value)}
          >
            {copy.projects.taskPriorities.map((label, level) => (
              <option value={level} key={label}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label htmlFor={`task-due-${task.id}`}>
          <span>{copy.projects.dueDateLabel}</span>
          <input
            id={`task-due-${task.id}`}
            type="date"
            value={dueDate}
            disabled={busy}
            onInput={(event) => setDueDate(event.currentTarget.value)}
          />
        </label>
      </div>
      {confirmingRemoval ? (
        <div
          className="project-task-edit-form__removal"
          role="group"
          aria-label={copy.projects.removeWorkItemConfirm}
        >
          <p>{copy.projects.removeWorkItemConfirm}</p>
          <div>
            <button
              ref={removeSafeActionRef}
              className="secondary-button focus-visible-control"
              type="button"
              disabled={busy}
              onClick={() => {
                restoreRemoveTriggerRef.current = true;
                setConfirmingRemoval(false);
              }}
            >
              {copy.projects.keepWorkItem}
            </button>
            <button
              className="destructive-button focus-visible-control"
              type="button"
              disabled={busy}
              onClick={() => void removeTask()}
            >
              {deleting ? (
                <span className="button-spinner" aria-hidden="true" />
              ) : (
                <Trash2 aria-hidden="true" />
              )}
              {deleting
                ? copy.projects.removingWorkItem
                : copy.projects.removeWorkItem}
            </button>
          </div>
        </div>
      ) : (
        <div className="project-task-edit-form__actions">
          <button
            ref={removeTriggerRef}
            className="destructive-quiet-button focus-visible-control"
            type="button"
            disabled={busy}
            onClick={() => setConfirmingRemoval(true)}
          >
            <Trash2 aria-hidden="true" />
            {copy.projects.removeWorkItem}
          </button>
          <span />
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={busy}
            onClick={onCancel}
          >
            {copy.projects.stopEditingWorkItem}
          </button>
          <button
            className="primary-button focus-visible-control"
            type="submit"
            disabled={busy}
          >
            {saving ? copy.actions.saving : copy.projects.saveWorkItem}
          </button>
        </div>
      )}
    </form>
  );
}

function WorkspaceTabsSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="workspace-tabs-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      <SkeletonBlock />
      <SkeletonBlock />
    </SkeletonGroup>
  );
}

function ProjectListSkeleton({
  rows,
  visible,
}: {
  rows: number;
  visible: boolean;
}) {
  return (
    <SkeletonGroup
      className="project-list-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      {Array.from({ length: rows }, (_, index) => (
        <span className="project-list-skeleton__row" key={index}>
          <span className="skeleton-copy-stack">
            <SkeletonBlock className="skeleton--title" />
            <SkeletonBlock className="skeleton--caption" />
          </span>
          <SkeletonBlock className="skeleton--project-meta" />
          <SkeletonBlock className="skeleton--chevron" />
        </span>
      ))}
    </SkeletonGroup>
  );
}

function ProjectDetailSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="project-detail-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      <span className="project-detail-skeleton__heading">
        <SkeletonBlock className="skeleton--label" />
        <SkeletonBlock className="skeleton--heading" />
        <SkeletonBlock className="skeleton--description" />
      </span>
      <span className="project-detail-skeleton__next-action">
        <SkeletonBlock className="skeleton--label" />
        <SkeletonBlock className="skeleton--title" />
      </span>
      <span className="project-detail-skeleton__tasks">
        <span className="project-detail-skeleton__section-heading">
          <SkeletonBlock className="skeleton--section-icon" />
          <SkeletonBlock className="skeleton--section-title" />
          <SkeletonBlock className="skeleton--count" />
        </span>
        {Array.from({ length: 3 }, (_, index) => (
          <span className="project-task-skeleton__row" key={index}>
            <SkeletonBlock className="skeleton--task-control" />
            <SkeletonBlock className="skeleton--task-title" />
          </span>
        ))}
        <span className="project-task-form-skeleton">
          <SkeletonBlock className="skeleton--field" />
          <SkeletonBlock className="skeleton--button" />
        </span>
      </span>
    </SkeletonGroup>
  );
}

function preferredScrollBehavior(): ScrollBehavior {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ? "auto"
    : "smooth";
}

function riskLabel(level: number): string {
  return copy.projects.riskLevels[level] || copy.projects.riskLevels[0];
}

function statusLabel(status: Project["status"]): string {
  return copy.projects.statuses[status];
}

function taskMeta(task: Task): string {
  const priority =
    copy.projects.taskPriorities[task.priority] ??
    copy.projects.taskPriorities[0];
  if (!task.dueAt) return priority;
  const date = new Date(task.dueAt);
  if (Number.isNaN(date.getTime())) return priority;
  return `${priority} · ${new Intl.DateTimeFormat("ko-KR", {
    month: "short",
    day: "numeric",
  }).format(date)}까지`;
}

function dateInputToIso(value: string): string | undefined {
  if (!value) return undefined;
  const due = new Date(`${value}T23:59:59`);
  return Number.isNaN(due.getTime()) ? undefined : due.toISOString();
}

function isoToDateInput(value: string | null): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function formatDueDate(value: string | null): string {
  if (!value) return copy.projects.noDueDate;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return copy.projects.noDueDate;
  return new Intl.DateTimeFormat("ko-KR", { dateStyle: "medium" }).format(date);
}
