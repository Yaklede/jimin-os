import {
  Check,
  ChevronRight,
  Flag,
  Pause,
  Pencil,
  Plus,
  RotateCcw,
  X,
} from "lucide-react";
import { FormEvent, useMemo, useState } from "react";

import { type Goal } from "../api/goals";
import { type Project } from "../api/projects";
import { copy } from "../copy";

type GoalInput = {
  title: string;
  desiredOutcome: string;
  projectId?: string;
  targetAt?: string;
};

export function GoalsPanel({
  goals,
  projects,
  workspaceId,
  saving,
  onCreate,
  onUpdate,
  onOpenTask,
  onOpenProject,
}: {
  goals: Goal[];
  projects: Project[];
  workspaceId: string | undefined;
  saving: boolean;
  onCreate(input: GoalInput): Promise<void>;
  onUpdate(
    goal: Goal,
    input: GoalInput & { status: Goal["status"] },
  ): Promise<void>;
  onOpenTask(taskId: string, projectId: string): void;
  onOpenProject(projectId: string): void;
}) {
  const [formOpen, setFormOpen] = useState(false);
  const [editingId, setEditingId] = useState<string>();
  const [title, setTitle] = useState("");
  const [outcome, setOutcome] = useState("");
  const [projectId, setProjectId] = useState("");
  const [targetDate, setTargetDate] = useState("");
  const [formError, setFormError] = useState<string>();
  const visibleGoals = useMemo(
    () => goals.filter((goal) => goal.workspaceId === workspaceId),
    [goals, workspaceId],
  );
  const activeGoals = visibleGoals.filter((goal) => goal.status === "active");
  const inactiveGoals = visibleGoals.filter((goal) => goal.status !== "active");

  function resetForm() {
    setEditingId(undefined);
    setTitle("");
    setOutcome("");
    setProjectId("");
    setTargetDate("");
    setFormError(undefined);
  }

  function openCreate() {
    resetForm();
    setFormOpen(true);
  }

  function openEdit(goal: Goal) {
    setEditingId(goal.id);
    setTitle(goal.title);
    setOutcome(goal.desiredOutcome);
    setProjectId(goal.projectId ?? "");
    setTargetDate(isoToDateInput(goal.targetAt));
    setFormError(undefined);
    setFormOpen(true);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim() || !outcome.trim()) {
      setFormError(copy.goals.requiredFields);
      return;
    }
    const input = {
      title: title.trim(),
      desiredOutcome: outcome.trim(),
      projectId: projectId || undefined,
      targetAt: dateInputToIso(targetDate),
    };
    try {
      const editing = visibleGoals.find((goal) => goal.id === editingId);
      if (editing) {
        await onUpdate(editing, { ...input, status: editing.status });
      } else {
        await onCreate(input);
      }
      resetForm();
      setFormOpen(false);
    } catch {
      setFormError(copy.goals.saveProblem);
    }
  }

  async function changeStatus(goal: Goal, status: Goal["status"]) {
    setFormError(undefined);
    try {
      await onUpdate(goal, {
        title: goal.title,
        desiredOutcome: goal.desiredOutcome,
        projectId: goal.projectId ?? undefined,
        targetAt: goal.targetAt ?? undefined,
        status,
      });
    } catch {
      setFormError(copy.goals.saveProblem);
    }
  }

  return (
    <section className="goals-panel" aria-labelledby="goals-panel-title">
      <header className="goals-panel__heading">
        <div>
          <span className="goals-panel__icon" aria-hidden="true">
            <Flag />
          </span>
          <div>
            <h2 id="goals-panel-title">{copy.goals.title}</h2>
            <p>{copy.goals.description}</p>
          </div>
        </div>
        <button
          className="secondary-button focus-visible-control"
          type="button"
          disabled={!workspaceId || saving}
          onClick={formOpen ? () => setFormOpen(false) : openCreate}
        >
          {formOpen ? <X aria-hidden="true" /> : <Plus aria-hidden="true" />}
          {formOpen ? copy.actions.cancel : copy.goals.create}
        </button>
      </header>

      {formOpen && (
        <form className="goal-form" onSubmit={(event) => void submit(event)}>
          <label htmlFor="goal-title">
            <span>{copy.goals.nameLabel}</span>
            <input
              id="goal-title"
              autoFocus
              value={title}
              maxLength={200}
              disabled={saving}
              placeholder={copy.goals.nameHint}
              onChange={(event) => setTitle(event.target.value)}
            />
          </label>
          <label htmlFor="goal-outcome">
            <span>{copy.goals.outcomeLabel}</span>
            <textarea
              id="goal-outcome"
              value={outcome}
              maxLength={2_000}
              rows={2}
              disabled={saving}
              placeholder={copy.goals.outcomeHint}
              onChange={(event) => setOutcome(event.target.value)}
            />
          </label>
          <div className="goal-form__split">
            <label htmlFor="goal-project">
              <span>{copy.goals.projectLabel}</span>
              <select
                id="goal-project"
                value={projectId}
                disabled={saving}
                onChange={(event) => setProjectId(event.target.value)}
              >
                <option value="">{copy.goals.noProject}</option>
                {projects.map((project) => (
                  <option value={project.id} key={project.id}>
                    {project.title}
                  </option>
                ))}
              </select>
            </label>
            <label htmlFor="goal-target-date">
              <span>{copy.goals.targetLabel}</span>
              <input
                id="goal-target-date"
                type="date"
                value={targetDate}
                disabled={saving}
                onInput={(event) => setTargetDate(event.currentTarget.value)}
              />
            </label>
          </div>
          {formError && (
            <p className="goal-form__error" role="alert">
              {formError}
            </p>
          )}
          <div className="goal-form__actions">
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={saving}
            >
              {saving
                ? copy.actions.saving
                : editingId
                  ? copy.goals.save
                  : copy.goals.create}
            </button>
          </div>
        </form>
      )}

      {!formOpen && formError && (
        <p className="goal-form__error" role="alert">
          {formError}
        </p>
      )}

      <div className="goals-panel__list">
        {activeGoals.length ? (
          activeGoals.map((goal) => (
            <GoalRow
              goal={goal}
              projects={projects}
              saving={saving}
              onEdit={() => openEdit(goal)}
              onPause={() => void changeStatus(goal, "paused")}
              onAchieve={() => void changeStatus(goal, "achieved")}
              onOpenTask={onOpenTask}
              onOpenProject={onOpenProject}
              key={goal.id}
            />
          ))
        ) : (
          <p className="goals-panel__empty">{copy.goals.empty}</p>
        )}
      </div>

      {inactiveGoals.length > 0 && (
        <details className="goals-panel__history">
          <summary>{copy.goals.history(inactiveGoals.length)}</summary>
          <ul>
            {inactiveGoals.map((goal) => (
              <li key={goal.id}>
                <span>
                  <strong>{goal.title}</strong>
                  <small>{goalStatusLabel(goal.status)}</small>
                </span>
                <button
                  className="icon-button focus-visible-control"
                  type="button"
                  disabled={saving}
                  aria-label={copy.goals.restore(goal.title)}
                  onClick={() => void changeStatus(goal, "active")}
                >
                  <RotateCcw aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}

function GoalRow({
  goal,
  projects,
  saving,
  onEdit,
  onPause,
  onAchieve,
  onOpenTask,
  onOpenProject,
}: {
  goal: Goal;
  projects: Project[];
  saving: boolean;
  onEdit(): void;
  onPause(): void;
  onAchieve(): void;
  onOpenTask(taskId: string, projectId: string): void;
  onOpenProject(projectId: string): void;
}) {
  const project = projects.find((item) => item.id === goal.projectId);
  const linkedProjectTitle = goal.projectTitle ?? project?.title;
  const canOpenNextAction = Boolean(
    goal.nextAction?.id &&
    (goal.nextAction.kind === "project" || goal.projectId),
  );

  function openNextAction() {
    const action = goal.nextAction;
    if (!action?.id) return;
    if (action.kind === "task" && goal.projectId) {
      onOpenTask(action.id, goal.projectId);
      return;
    }
    onOpenProject(action.id);
  }

  return (
    <article className="goal-row" data-health={goal.health}>
      <span className="goal-row__marker" aria-hidden="true" />
      <div className="goal-row__content">
        <div className="goal-row__title-line">
          <strong>{goal.title}</strong>
          <span className="goal-row__health">
            {goalHealthLabel(goal.health)}
          </span>
        </div>
        <p>{goal.desiredOutcome}</p>
        <span className="goal-row__scope">
          {linkedProjectTitle ?? copy.goals.noProject}
          {goal.targetAt ? ` · ${formatTarget(goal.targetAt)}` : ""}
        </span>
        <div className="goal-row__progress">
          <div>
            <span>{copy.goals.progress}</span>
            <strong>{goal.progressPercent}%</strong>
          </div>
          <span
            className="goal-row__progress-track"
            role="progressbar"
            aria-label={copy.goals.progressFor(goal.title)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={goal.progressPercent}
          >
            <span style={{ width: `${goal.progressPercent}%` }} />
          </span>
        </div>
        <div className="goal-row__evidence" aria-label={copy.goals.evidence}>
          <span>
            {copy.goals.completedEvidence(
              goal.completedTaskCount,
              goal.totalTaskCount,
            )}
          </span>
          <span>{copy.goals.weeklyEvidence(goal.completedLastSevenDays)}</span>
          {goal.overdueTaskCount > 0 && (
            <span data-risk="true">
              {copy.goals.overdueEvidence(goal.overdueTaskCount)}
            </span>
          )}
        </div>
        {goal.nextAction && (
          <button
            className="goal-row__next focus-visible-control"
            type="button"
            disabled={!canOpenNextAction}
            onClick={openNextAction}
          >
            <span>
              <small>{copy.goals.nextAction}</small>
              <strong>{goal.nextAction.title}</strong>
            </span>
            {canOpenNextAction && <ChevronRight aria-hidden="true" />}
          </button>
        )}
      </div>
      <div className="goal-row__actions">
        <button
          className="icon-button focus-visible-control"
          type="button"
          disabled={saving}
          aria-label={copy.goals.edit(goal.title)}
          onClick={onEdit}
        >
          <Pencil aria-hidden="true" />
        </button>
        <button
          className="icon-button focus-visible-control"
          type="button"
          disabled={saving}
          aria-label={copy.goals.pause(goal.title)}
          onClick={onPause}
        >
          <Pause aria-hidden="true" />
        </button>
        <button
          className="icon-button goal-row__complete focus-visible-control"
          type="button"
          disabled={saving}
          aria-label={copy.goals.achieve(goal.title)}
          onClick={onAchieve}
        >
          <Check aria-hidden="true" />
        </button>
      </div>
    </article>
  );
}

function goalHealthLabel(health: Goal["health"]): string {
  if (health === "at_risk") return copy.goals.healthAtRisk;
  if (health === "needs_plan") return copy.goals.healthNeedsPlan;
  if (health === "ready_to_complete") return copy.goals.healthReady;
  if (health === "paused") return copy.goals.paused;
  if (health === "achieved") return copy.goals.achieved;
  return copy.goals.healthOnTrack;
}

function dateInputToIso(value: string): string | undefined {
  if (!value) return undefined;
  const parsed = new Date(`${value}T23:59:59`);
  return Number.isNaN(parsed.getTime()) ? undefined : parsed.toISOString();
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

function formatTarget(value: string): string {
  const date = new Date(value);
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
  }).format(date);
}

function goalStatusLabel(status: Goal["status"]): string {
  if (status === "paused") return copy.goals.paused;
  if (status === "achieved") return copy.goals.achieved;
  if (status === "cancelled") return copy.goals.cancelled;
  return copy.goals.active;
}
