import type { ProjectInflowItem } from "./api/googleChat";
import type { Task } from "./api/planning";

// Decisions affect only the synthetic data owned by this preview session.
export function applyDesignPreviewInflowDecision(
  item: ProjectInflowItem,
  tasks: Task[],
  input: unknown,
): Response {
  const invalid = () =>
    Response.json({ error: "preview_invalid_decision" }, { status: 400 });
  if (!input || typeof input !== "object") return invalid();
  const values = input as Record<string, unknown>;
  if (item.status !== "pending" || values.expectedVersion !== item.version) {
    return Response.json(
      { error: "preview_decision_conflict" },
      { status: 409 },
    );
  }
  if (values.decision === "dismiss") {
    item.status = "dismissed";
    item.version += 1;
    return Response.json(item);
  }
  if (values.decision !== "promote") return invalid();
  if (
    values.conversationId !== item.conversationId ||
    values.representativeItemId !== item.representativeItemId ||
    values.expectedSourceRevision !== item.sourceRevision ||
    values.expectedAnalyzedRevision !== item.analyzedRevision
  ) {
    return Response.json(
      { error: "preview_analysis_changed" },
      { status: 409 },
    );
  }
  if (
    typeof values.title !== "string" ||
    !values.title.trim() ||
    typeof values.notes !== "string" ||
    ![1, 2, 3].includes(Number(values.priority)) ||
    (values.assigneeName !== undefined &&
      (typeof values.assigneeName !== "string" ||
        !item.assigneeOptions.includes(values.assigneeName))) ||
    (values.withoutDeadline !== true &&
      (values.withoutDeadline !== false ||
        typeof values.dueAt !== "string" ||
        !Number.isFinite(Date.parse(values.dueAt))))
  )
    return invalid();

  const task: Task = {
    id: `preview-inflow-task-${item.id}`,
    projectId: item.projectId,
    title: values.title.trim(),
    notes: values.notes.trim(),
    assigneeName:
      typeof values.assigneeName === "string" ? values.assigneeName : null,
    status: "open",
    priority: Number(values.priority),
    dueAt: values.withoutDeadline === true ? null : String(values.dueAt),
    completedAt: null,
    version: 1,
  };
  tasks.push(task);
  item.status = "promoted";
  item.promotedTaskId = task.id;
  item.version += 1;
  // The preview never claims to have sent notifications or changed Chat messages.
  item.completionStatus = "not_requested";
  return Response.json(item);
}
