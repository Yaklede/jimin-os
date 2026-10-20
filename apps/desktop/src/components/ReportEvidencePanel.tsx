import { useEffect, useRef, useState } from "react";
import type { ReportEvidenceMetric, ReportEvidencePage } from "../api/projects";
import type { Task } from "../api/planning";

export const reportEvidenceLabels: Record<ReportEvidenceMetric, string> = {
  created: "이번 주 들어온 일",
  completed: "이번 주 완료한 일",
  backlog: "남은 일",
  overdue: "기한 지난 일",
  stale: "오래 변경되지 않은 일",
  unassigned: "담당자 없는 일",
  waiting: "응답을 기다리는 일",
  on_time: "마감일이 있는 완료 일감",
};
type Props = {
  metric: ReportEvidenceMetric;
  onLoad(
    metric: ReportEvidenceMetric,
    after?: string,
  ): Promise<ReportEvidencePage>;
  onOpen(task: Task): void;
  onClose(): void;
};
export function ReportEvidencePanel({
  metric,
  onLoad,
  onOpen,
  onClose,
}: Props) {
  const [page, setPage] = useState<ReportEvidencePage>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const generation = useRef(0);
  const loadRef = useRef(onLoad);
  loadRef.current = onLoad;
  async function load(after?: string) {
    const id = ++generation.current;
    setBusy(true);
    setError("");
    try {
      const next = await loadRef.current(metric, after);
      if (generation.current === id)
        setPage((previous) => ({
          items: after
            ? [...(previous?.items ?? []), ...next.items]
            : next.items,
          nextCursor: next.nextCursor,
        }));
    } catch {
      if (generation.current === id)
        setError("관련 일감을 불러오지 못했어요. 다시 시도해 주세요.");
    } finally {
      if (generation.current === id) setBusy(false);
    }
  }
  useEffect(() => {
    setPage(undefined);
    void load();
    return () => {
      generation.current++;
    };
  }, [metric]);
  return (
    <section
      className="report-evidence"
      aria-busy={busy}
      aria-label={reportEvidenceLabels[metric]}
    >
      <header>
        <h4>{reportEvidenceLabels[metric]}</h4>
        <button
          type="button"
          className="secondary-button focus-visible-control"
          onClick={onClose}
        >
          접기
        </button>
      </header>
      <p>
        같은 집계 기준에 해당하는 일감의 현재 상태예요. 상위 일감에 하위 일감이
        있으면 하위 일감만 집계해요.
      </p>
      {error && (
        <p role="alert">
          {error}{" "}
          <button type="button" disabled={busy} onClick={() => void load()}>
            다시 불러오기
          </button>
        </p>
      )}
      {busy && !page && <p role="status">관련 일감을 불러오고 있어요.</p>}
      {page?.items.length === 0 && <p>현재 해당하는 일감이 없어요.</p>}
      <ul>
        {page?.items.map((task) => (
          <li key={task.id}>
            <button
              type="button"
              className="focus-visible-control"
              onClick={() => onOpen(task)}
            >
              <strong>{task.title}</strong>
              <span>
                {task.assigneeName || "담당자 미정"} ·{" "}
                {task.status === "completed"
                  ? "완료"
                  : task.workflowState === "waiting"
                    ? "응답 대기"
                    : task.workflowState === "in_progress"
                      ? "진행 중"
                      : "진행 전"}
              </span>
              {task.waitingReason && task.status === "open" && (
                <small>대기 사유: {task.waitingReason}</small>
              )}
            </button>
          </li>
        ))}
      </ul>
      {page?.nextCursor && (
        <button
          type="button"
          className="secondary-button focus-visible-control"
          disabled={busy}
          onClick={() => void load(page.nextCursor!)}
        >
          일감 더 보기
        </button>
      )}
    </section>
  );
}
