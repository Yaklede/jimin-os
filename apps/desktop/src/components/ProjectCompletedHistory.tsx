import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, Search } from "lucide-react";
import type { Task } from "../api/planning";
import type {
  ProjectCompletedPage,
  ProjectCompletedQuery,
} from "../api/projects";
import { projectCompletedHistoryCopy as copy } from "../copy/projectCompletedHistory";
import "./projectCompletedHistory.css";

type Loader = (
  projectId: string,
  query: ProjectCompletedQuery,
  signal: AbortSignal,
) => Promise<ProjectCompletedPage>;

const emptyFilters = {
  query: "",
  assignee: "",
  completedFrom: "",
  completedTo: "",
};

export function ProjectCompletedHistory({
  projectId,
  count,
  revision,
  anchorId,
  onLoad,
  renderRows,
}: {
  projectId: string;
  count: number;
  revision: number;
  anchorId?: string;
  onLoad: Loader;
  renderRows(items: Task[], refresh: () => void): ReactNode;
}) {
  const [draft, setDraft] = useState(emptyFilters);
  const [request, setRequest] = useState<ProjectCompletedQuery>({
    page: 1,
    anchorTaskId: anchorId,
  });
  const [data, setData] = useState<ProjectCompletedPage>();
  const [loading, setLoading] = useState(true);
  const [loadProblem, setLoadProblem] = useState(false);
  const [dateProblem, setDateProblem] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const refresh = () => {
    setLoading(true);
    setRefreshKey((current) => current + 1);
  };
  const filtered = Boolean(
    request.query ||
    request.assignee ||
    request.completedFrom ||
    request.completedTo,
  );
  const hasDraft = Object.values(draft).some(Boolean);
  const shown = data?.page ?? 1;
  const last = data
    ? Math.max(1, Math.ceil(data.filteredCount / data.pageSize))
    : 1;

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setLoadProblem(false);
    void onLoad(projectId, request, controller.signal)
      .then((page) => {
        if (!controller.signal.aborted) setData(page);
      })
      .catch(() => {
        if (!controller.signal.aborted) setLoadProblem(true);
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [onLoad, projectId, refreshKey, request, revision]);

  useEffect(() => {
    if (!data || loading || !request.anchorTaskId) return;
    document
      .getElementById(`project-completed-task-${request.anchorTaskId}`)
      ?.scrollIntoView({ block: "nearest" });
  }, [data, loading, request.anchorTaskId]);

  function search(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      draft.completedFrom &&
      draft.completedTo &&
      draft.completedFrom > draft.completedTo
    ) {
      setDateProblem(true);
      return;
    }
    setDateProblem(false);
    setLoading(true);
    setRequest({ ...draft, query: draft.query.trim(), page: 1 });
  }

  function page(pageNumber: number) {
    if (loading || loadProblem || pageNumber < 1 || pageNumber > last) return;
    setLoading(true);
    setRequest((current) => ({
      ...current,
      anchorTaskId: undefined,
      page: pageNumber,
    }));
  }

  return (
    <section
      className="project-completed-tasks"
      aria-labelledby="completed-tasks-title"
    >
      <div className="projects-section-heading">
        <h3 id="completed-tasks-title">{copy.heading}</h3>
        <span className="project-history__count">
          {copy.total(data?.totalCount ?? count)}
        </span>
      </div>
      <form
        className="project-history__form"
        aria-label={copy.filters}
        onSubmit={search}
      >
        <label className="project-history__query">
          <span>{copy.queryLabel}</span>
          <input
            type="search"
            name="query"
            value={draft.query}
            maxLength={240}
            placeholder={copy.queryHint}
            onChange={(event) =>
              setDraft((current) => ({ ...current, query: event.target.value }))
            }
          />
        </label>
        <label>
          <span>{copy.assigneeLabel}</span>
          <select
            name="assignee"
            value={draft.assignee}
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                assignee: event.target.value,
              }))
            }
          >
            <option value="">{copy.allAssignees}</option>
            {draft.assignee && !data?.assignees.includes(draft.assignee) && (
              <option value={draft.assignee}>{draft.assignee}</option>
            )}
            {data?.assignees.map((assignee) => (
              <option key={assignee} value={assignee}>
                {assignee}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{copy.fromLabel}</span>
          <input
            type="date"
            name="completedFrom"
            value={draft.completedFrom}
            aria-invalid={dateProblem || undefined}
            onChange={(event) => {
              setDateProblem(false);
              setDraft((current) => ({
                ...current,
                completedFrom: event.target.value,
              }));
            }}
          />
        </label>
        <label>
          <span>{copy.toLabel}</span>
          <input
            type="date"
            name="completedTo"
            value={draft.completedTo}
            aria-invalid={dateProblem || undefined}
            onChange={(event) => {
              setDateProblem(false);
              setDraft((current) => ({
                ...current,
                completedTo: event.target.value,
              }));
            }}
          />
        </label>
        <div className="project-history__actions">
          <button
            type="submit"
            className="secondary-button focus-visible-control"
          >
            <Search aria-hidden="true" /> {copy.search}
          </button>
          {(hasDraft || filtered) && (
            <button
              type="button"
              className="text-button focus-visible-control"
              onClick={() => {
                setDateProblem(false);
                setDraft(emptyFilters);
                setLoading(true);
                setRequest({ page: 1 });
              }}
            >
              {copy.reset}
            </button>
          )}
        </div>
        {dateProblem && (
          <p className="inline-alert project-history__form-alert" role="alert">
            {copy.invalidDate}
          </p>
        )}
      </form>
      {loadProblem && (
        <div className="inline-alert project-history__problem" role="alert">
          <span>{copy.loadProblem}</span>
          <button
            className="text-button focus-visible-control"
            type="button"
            onClick={refresh}
          >
            {copy.retry}
          </button>
        </div>
      )}
      <div className="project-history__controls">
        <p className="project-history__status" role="status">
          {loading
            ? copy.loading
            : !loadProblem && filtered && data
              ? copy.matched(data.filteredCount)
              : ""}
        </p>
        {data && data.filteredCount > 0 && (
          <nav className="project-history__pages" aria-label={copy.navigation}>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              aria-disabled={loading || loadProblem || shown <= 1}
              onClick={() => page(shown - 1)}
            >
              <ChevronLeft aria-hidden="true" /> {copy.previous}
            </button>
            <span aria-live="polite">{copy.pageLabel(shown, last)}</span>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              aria-disabled={loading || loadProblem || shown >= last}
              onClick={() => page(shown + 1)}
            >
              {copy.next} <ChevronRight aria-hidden="true" />
            </button>
          </nav>
        )}
      </div>
      <div
        aria-busy={loading}
        inert={loading}
        className="project-history__rows"
      >
        {!loadProblem &&
          (data?.items.length
            ? renderRows(data.items, refresh)
            : !loading && (
                <p className="project-detail__empty">
                  {filtered ? copy.noMatches : copy.empty}
                </p>
              ))}
      </div>
    </section>
  );
}
