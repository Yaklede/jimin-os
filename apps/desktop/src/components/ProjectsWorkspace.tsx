import {
  BriefcaseBusiness,
  ChevronRight,
  Circle,
  CircleAlert,
  FolderKanban,
  ListTodo,
  Plus,
} from "lucide-react";
import { FormEvent, useEffect, useState } from "react";

import { type Project, type Workspace } from "../api/projects";
import { type Task } from "../api/planning";
import { copy } from "../copy";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import { EmptySurface } from "./HomeWorkspace";

type ProjectsWorkspaceProps = {
  workspaces: Workspace[];
  projects: Project[];
  tasks: Task[];
  selectedWorkspaceId: string | undefined;
  selectedProjectId: string | undefined;
  loading: boolean;
  saving: boolean;
  error: string | undefined;
  onSelectWorkspace(workspaceId: string): void;
  onSelectProject(projectId: string): void;
  onCreateProject(input: {
    title: string;
    objective?: string;
    riskLevel: number;
    nextAction?: string;
  }): Promise<void>;
  onCreateTask(title: string): Promise<void>;
  onCompleteTask(task: Task): Promise<void>;
};

export function ProjectsWorkspace({
  workspaces,
  projects,
  tasks,
  selectedWorkspaceId,
  selectedProjectId,
  loading,
  saving,
  error,
  onSelectWorkspace,
  onSelectProject,
  onCreateProject,
  onCreateTask,
  onCompleteTask,
}: ProjectsWorkspaceProps) {
  const [formOpen, setFormOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [objective, setObjective] = useState("");
  const [nextAction, setNextAction] = useState("");
  const [riskLevel, setRiskLevel] = useState("0");
  const [taskTitle, setTaskTitle] = useState("");
  const [formError, setFormError] = useState<string>();
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;

  const selectedProject = projects.find(
    (project) => project.id === selectedProjectId,
  );

  useEffect(() => {
    setTaskTitle("");
  }, [selectedProjectId]);

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
        riskLevel: Number(riskLevel),
        nextAction: nextAction.trim() || undefined,
      });
      setTitle("");
      setObjective("");
      setNextAction("");
      setRiskLevel("0");
      setFormOpen(false);
    } catch {
      setFormError(copy.projects.projectSaveNotice);
    }
  }

  async function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!taskTitle.trim()) return;
    try {
      await onCreateTask(taskTitle.trim());
      setTaskTitle("");
    } catch {
      setFormError(copy.projects.taskSaveNotice);
    }
  }

  return (
    <section className="projects-page" aria-busy={showingSkeleton || saving}>
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
          <div className="project-create-form__split">
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

      <div className="projects-layout">
        <section
          className="projects-list"
          aria-labelledby="projects-list-title"
        >
          <div className="projects-section-heading">
            <div>
              <FolderKanban aria-hidden="true" />
              <h2 id="projects-list-title">{copy.projects.listTitle}</h2>
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
                      onClick={() => onSelectProject(project.id)}
                    >
                      <span className="project-list__main">
                        <strong>{project.title}</strong>
                        <span>
                          {project.nextAction ||
                            project.objective ||
                            copy.projects.noNextAction}
                        </span>
                      </span>
                      <span className="project-list__meta">
                        {project.riskLevel > 0 && (
                          <span data-risk={project.riskLevel}>
                            <CircleAlert aria-hidden="true" />
                            {riskLabel(project.riskLevel)}
                          </span>
                        )}
                        <span>
                          {copy.projects.openTaskCount(project.openTaskCount)}
                        </span>
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
            <ProjectDetailSkeleton visible={skeletonVisible} />
          ) : selectedProject ? (
            <>
              <div className="project-detail__heading">
                <div>
                  <p>{copy.projects.nextActionLabel}</p>
                  <h2 id="project-detail-title">{selectedProject.title}</h2>
                  <span>
                    {selectedProject.objective || copy.projects.objectiveEmpty}
                  </span>
                </div>
                {selectedProject.riskLevel > 0 && (
                  <span
                    className="project-risk"
                    data-risk={selectedProject.riskLevel}
                  >
                    <CircleAlert aria-hidden="true" />
                    {riskLabel(selectedProject.riskLevel)}
                  </span>
                )}
              </div>
              <div className="project-next-action">
                <span>{copy.projects.nextActionLabel}</span>
                <strong>
                  {selectedProject.nextAction || copy.projects.noNextAction}
                </strong>
              </div>
              <div className="project-detail__tasks">
                <div className="projects-section-heading">
                  <div>
                    <ListTodo aria-hidden="true" />
                    <h3>{copy.projects.workItemsTitle}</h3>
                  </div>
                  <span>{copy.projects.openTaskCount(tasks.length)}</span>
                </div>
                {tasks.length ? (
                  <ul className="project-task-list">
                    {tasks.map((task) => (
                      <li key={task.id}>
                        <button
                          className="project-task-list__complete focus-visible-control"
                          type="button"
                          disabled={saving}
                          aria-label={copy.home.completeTask(task.title)}
                          onClick={() => void onCompleteTask(task)}
                        >
                          <Circle aria-hidden="true" />
                        </button>
                        <span>{task.title}</span>
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
                    disabled={saving}
                    maxLength={200}
                    placeholder={copy.projects.workItemHint}
                  />
                  <button
                    className="secondary-button focus-visible-control"
                    type="submit"
                    disabled={saving || !taskTitle.trim()}
                  >
                    {copy.actions.addWorkItem}
                  </button>
                </form>
              </div>
            </>
          ) : (
            <EmptySurface
              title={copy.projects.selectTitle}
              description={copy.projects.selectDescription}
            />
          )}
        </section>
      </div>
    </section>
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

function riskLabel(level: number): string {
  return copy.projects.riskLevels[level] || copy.projects.riskLevels[0];
}
