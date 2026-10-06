import { useEffect, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, Pencil, RefreshCw } from "lucide-react";
import type { Task } from "../api/planning";
import {
  filterMobileTasks,
  groupMobileTasks,
  mergeMobileTasks,
  type MobileTaskFilter,
  type MobileTaskGrouping,
} from "../mobileTaskView";
import { LinkifiedText } from "./ExternalTextLink";

type Props = {
  tasks: Task[];
  loading?: boolean;
  onLoadAll(): Promise<Task[]>;
  onComplete(task: Task): Promise<void>;
  onEdit(task: Task): void;
};

export function MobileTaskQueue({
  tasks,
  loading,
  onLoadAll,
  onComplete,
  onEdit,
}: Props) {
  const [filter, setFilter] = useState<MobileTaskFilter>("today");
  const [grouping, setGrouping] = useState<MobileTaskGrouping>("assignee");
  const [allTasks, setAllTasks] = useState<Task[]>([]);
  const [fetching, setFetching] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const [expanded, setExpanded] = useState<string>();
  const [pending, setPending] = useState<string>();
  const [now, setNow] = useState(() => new Date());
  const generation = useRef(0);
  const loadRef = useRef(onLoadAll);
  loadRef.current = onLoadAll;
  async function reload() {
    const request = ++generation.current;
    setFetching(true);
    setError("");
    try {
      const next = await loadRef.current();
      if (request !== generation.current) return;
      setAllTasks(next);
      setLoaded(true);
    } catch {
      if (request === generation.current)
        setError("전체 일감을 불러오지 못했어요. 다시 불러와 주세요.");
    } finally {
      if (request === generation.current) setFetching(false);
    }
  }
  useEffect(() => {
    void reload();
    const interval = window.setInterval(() => setNow(new Date()), 60_000);
    return () => {
      generation.current++;
      window.clearInterval(interval);
    };
  }, []);
  const previousTasks = useRef(tasks);
  useEffect(() => {
    if (previousTasks.current !== tasks) {
      previousTasks.current = tasks;
      void reload();
    }
  }, [tasks]);
  const merged = useMemo(
    () => mergeMobileTasks(allTasks, tasks, loaded),
    [allTasks, tasks, loaded],
  );
  const visible = filterMobileTasks(merged, filter, now);
  const groups = groupMobileTasks(visible, grouping);
  async function complete(task: Task) {
    if (pending) return;
    setPending(task.id);
    setError("");
    try {
      await onComplete(task);
      await reload();
    } catch {
      setError("완료 상태를 저장하지 못했어요. 다시 시도해 주세요.");
    } finally {
      setPending(undefined);
    }
  }
  return (
    <section
      className="mobile-task-queue"
      aria-labelledby="mobile-task-queue-title"
    >
      <header className="mobile-task-queue__heading">
        <h2 id="mobile-task-queue-title">할 일</h2>
        <button
          type="button"
          className="focus-visible-control"
          aria-label="일감 다시 불러오기"
          disabled={fetching}
          onClick={() => void reload()}
        >
          <RefreshCw className={fetching ? "spin" : ""} aria-hidden="true" />
        </button>
      </header>
      <div className="mobile-task-queue__filters" aria-label="일감 범위">
        {(
          [
            ["today", "오늘"],
            ["overdue", "기한 지남"],
            ["all", "전체"],
          ] as const
        ).map(([value, label]) => (
          <button
            type="button"
            key={value}
            className="focus-visible-control"
            aria-pressed={filter === value}
            onClick={() => setFilter(value)}
          >
            {label}
            <span>
              {loaded ? filterMobileTasks(merged, value, now).length : "—"}
            </span>
          </button>
        ))}
      </div>
      <div className="mobile-task-queue__grouping" aria-label="일감 보기 기준">
        {(
          [
            ["assignee", "담당자별"],
            ["date", "일자별"],
          ] as const
        ).map(([value, label]) => (
          <button
            type="button"
            key={value}
            className="focus-visible-control"
            aria-pressed={grouping === value}
            onClick={() => setGrouping(value)}
          >
            {label}
          </button>
        ))}
      </div>
      <p className="mobile-task-queue__status" role="status">
        {fetching || loading
          ? "일감을 확인하고 있어요."
          : `${visible.length}개 · ${filter === "today" ? "오늘 마감인 일" : filter === "overdue" ? "마감 시각이 지난 일" : "완료하지 않은 모든 일"}`}
      </p>
      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}
      {groups.map(([label, items]) => (
        <section className="mobile-task-group" key={label} aria-label={label}>
          <h3>
            {grouping === "date" && label !== "기한 없음"
              ? new Intl.DateTimeFormat("ko-KR", {
                  timeZone: "Asia/Seoul",
                  month: "long",
                  day: "numeric",
                  weekday: "short",
                }).format(new Date(`${label}T00:00:00+09:00`))
              : label}
            <span>{items.length}개</span>
          </h3>
          <ul>
            {items.map((task) => (
              <li key={task.id}>
                <div className="mobile-task-row">
                  <button
                    type="button"
                    className="mobile-task-row__complete focus-visible-control"
                    aria-label={`${task.title} 완료하기`}
                    disabled={Boolean(pending)}
                    onClick={() => void complete(task)}
                  >
                    {pending === task.id ? (
                      <span className="button-spinner" aria-hidden="true" />
                    ) : (
                      <span
                        className="mobile-task-row__circle"
                        aria-hidden="true"
                      />
                    )}
                  </button>
                  <button
                    type="button"
                    className="mobile-task-row__open focus-visible-control"
                    aria-expanded={expanded === task.id}
                    aria-controls={`mobile-task-${task.id}`}
                    onClick={() =>
                      setExpanded(expanded === task.id ? undefined : task.id)
                    }
                  >
                    <strong>{task.title}</strong>
                    <span>
                      {grouping === "date"
                        ? task.assigneeName || "담당자 미정"
                        : dueLabel(task.dueAt)}
                      {task.parentTaskId ? " · 하위 일감" : ""}
                    </span>
                    <ChevronDown aria-hidden="true" />
                  </button>
                </div>
                {expanded === task.id && (
                  <div
                    className="mobile-task-detail"
                    id={`mobile-task-${task.id}`}
                  >
                    <p>
                      {task.assigneeName || "담당자 미정"} ·{" "}
                      {dueLabel(task.dueAt)}
                    </p>
                    {task.notes ? (
                      <div className="mobile-task-detail__notes">
                        <LinkifiedText text={task.notes} />
                      </div>
                    ) : (
                      <p>설명이 없어요. 수정에서 추가할 수 있어요.</p>
                    )}
                    <div className="mobile-task-detail__actions">
                      <button
                        type="button"
                        className="secondary-button focus-visible-control"
                        onClick={() => onEdit(task)}
                      >
                        <Pencil aria-hidden="true" />
                        수정하기
                      </button>
                      <button
                        type="button"
                        className="primary-button focus-visible-control"
                        disabled={Boolean(pending)}
                        onClick={() => void complete(task)}
                      >
                        <Check aria-hidden="true" />
                        {pending === task.id ? "저장 중" : "완료하기"}
                      </button>
                    </div>
                  </div>
                )}
              </li>
            ))}
          </ul>
        </section>
      ))}
      {loaded && !fetching && !visible.length && (
        <p className="mobile-task-queue__empty">
          {filter === "today"
            ? "오늘 마감인 일이 없어요. 전체에서 다른 일을 확인해 보세요."
            : filter === "overdue"
              ? "기한이 지난 일이 없어요."
              : "남은 일이 없어요."}
        </p>
      )}
    </section>
  );
}

function dueLabel(dueAt: string | null) {
  return dueAt
    ? new Intl.DateTimeFormat("ko-KR", {
        timeZone: "Asia/Seoul",
        month: "numeric",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      }).format(new Date(dueAt))
    : "기한 없음";
}
