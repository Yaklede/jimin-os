import { useEffect, useMemo, useRef, useState, type Ref } from "react";
import {
  Check,
  ChevronDown,
  Pencil,
  RefreshCw,
  SlidersHorizontal,
} from "lucide-react";
import type { Task } from "../api/planning";
import {
  filterMobileTasks,
  groupMobileTasks,
  mergeMobileTasks,
  taskAssignees,
  type MobileTaskFilter,
  type MobileTaskGrouping,
} from "../mobileTaskView";
import { LinkifiedText } from "./ExternalTextLink";
import { workspaceActionCopy as text } from "../copy/workspaceActions";
import "./homeTaskQueue.css";

type Props = {
  sectionRef?: Ref<HTMLElement>;
  todayRequest?: number;
  tasks: Task[];
  loading?: boolean;
  onLoadAll(): Promise<Task[]>;
  onComplete(task: Task): Promise<void>;
  onEdit(task: Task): void;
};

export function MobileTaskQueue({
  sectionRef,
  todayRequest = 0,
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
  const [workflowFilter, setWorkflowFilter] = useState("all");
  const [now, setNow] = useState(() => new Date());
  const [filtersOpen, setFiltersOpen] = useState(false);
  useEffect(() => {
    if (todayRequest > 0) {
      setFilter("today");
      setWorkflowFilter("all");
      setFiltersOpen(false);
    }
  }, [todayRequest]);
  const generation = useRef(0);
  const mounted = useRef(true);
  const heading = useRef<HTMLHeadingElement>(null);
  const rows = useRef<HTMLDivElement>(null);
  const completionFocus = useRef<HTMLElement | null>(null);
  const handledTodayRequest = useRef(0);
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
      return next;
    } catch {
      if (request === generation.current)
        setError("전체 일감을 불러오지 못했어요. 다시 불러와 주세요.");
    } finally {
      if (request === generation.current) setFetching(false);
    }
  }
  useEffect(() => {
    mounted.current = true;
    void reload();
    const interval = window.setInterval(() => setNow(new Date()), 60_000);
    return () => {
      mounted.current = false;
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
  const visible = filterMobileTasks(merged, filter, now).filter(
    (task) =>
      workflowFilter === "all" ||
      (task.workflowState ?? "not_started") === workflowFilter,
  );
  const groups = groupMobileTasks(visible, grouping);
  useEffect(() => {
    if (
      !todayRequest ||
      handledTodayRequest.current === todayRequest ||
      filter !== "today" ||
      workflowFilter !== "all" ||
      filtersOpen ||
      (!visible.length && (fetching || loading))
    )
      return;
    const target =
      rows.current?.querySelector<HTMLButtonElement>(
        ".mobile-task-row__open",
      ) ?? heading.current;
    if (!target) return;
    handledTodayRequest.current = todayRequest;
    target.scrollIntoView({
      block: "center",
      inline: "nearest",
      behavior: "instant",
    });
    target.focus({ preventScroll: true });
  }, [
    todayRequest,
    filter,
    workflowFilter,
    filtersOpen,
    fetching,
    loading,
    visible.length,
  ]);
  useEffect(() => {
    if (pending || !completionFocus.current) return;
    const target = completionFocus.current;
    completionFocus.current = null;
    if (target.isConnected) target.focus({ preventScroll: true });
    else heading.current?.focus({ preventScroll: true });
  }, [pending]);

  async function complete(task: Task, trigger: HTMLButtonElement) {
    if (pending) return;
    setPending(task.id);
    setError("");
    try {
      await onComplete(task);
      if (!mounted.current) return;
      const updated = await reload();
      if (!mounted.current) return;
      if (
        updated &&
        !updated.some((item) => item.id === task.id && item.status === "open")
      ) {
        setExpanded(undefined);
        completionFocus.current = heading.current;
      } else completionFocus.current = trigger;
    } catch {
      if (mounted.current) {
        completionFocus.current = trigger;
        setError("완료 상태를 저장하지 못했어요. 다시 시도해 주세요.");
      }
    } finally {
      if (mounted.current) setPending(undefined);
    }
  }
  return (
    <section
      ref={sectionRef}
      id="home-today-task-list"
      tabIndex={-1}
      className="mobile-task-queue home-direct-tasks"
      aria-labelledby="mobile-task-queue-title"
    >
      <header className="mobile-task-queue__heading">
        <h2 ref={heading} tabIndex={-1} id="mobile-task-queue-title">
          할 일{" "}
          <small>{fetching && !loaded ? "" : `${visible.length}개`}</small>
        </h2>
      </header>
      <div className="home-direct-tasks__tools">
        <button
          className="home-direct-tasks__today focus-visible-control"
          type="button"
          aria-pressed={filter === "today" && workflowFilter === "all"}
          onClick={() => {
            setFilter("today");
            setWorkflowFilter("all");
          }}
        >
          오늘
        </button>
        <div
          className="mobile-task-queue__grouping"
          role="group"
          aria-label="일감 보기 기준"
        >
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
        <button
          className="home-direct-tasks__filters focus-visible-control"
          type="button"
          aria-expanded={filtersOpen}
          aria-controls="home-task-filter-controls"
          data-filter-active={filter !== "today" || workflowFilter !== "all"}
          onClick={() => setFiltersOpen((current) => !current)}
        >
          <SlidersHorizontal aria-hidden="true" />
          {text.taskFilters}
          {(filter !== "today" || workflowFilter !== "all") && (
            <span className="sr-only">{text.taskFilterActive}</span>
          )}
        </button>
      </div>
      <div
        className="home-direct-tasks__filter-body"
        id="home-task-filter-controls"
        hidden={!filtersOpen}
      >
        <div
          className="mobile-task-queue__filters"
          role="group"
          aria-label="일감 범위"
        >
          {(
            [
              ["today", "오늘"],
              ["overdue", "기한 지남"],
              ["all", "전체"],
            ] as const
          ).map(([value, label]) => (
            <button
              className="focus-visible-control"
              type="button"
              key={value}
              aria-pressed={filter === value}
              onClick={() => setFilter(value)}
            >
              {label}{" "}
              <span>
                {loaded ? filterMobileTasks(merged, value, now).length : "—"}
              </span>
            </button>
          ))}
        </div>
        <label className="home-direct-tasks__workflow">
          진행 상태
          <select
            aria-label="진행 상태"
            value={workflowFilter}
            onChange={(event) => setWorkflowFilter(event.target.value)}
          >
            <option value="all">모든 상태</option>
            <option value="not_started">진행 전</option>
            <option value="in_progress">진행 중</option>
            <option value="waiting">대기 중</option>
          </select>
        </label>
        <button
          type="button"
          className="text-button focus-visible-control"
          disabled={fetching}
          onClick={() => void reload()}
        >
          <RefreshCw className={fetching ? "spin" : ""} aria-hidden="true" />{" "}
          다시 불러오기
        </button>
      </div>
      <p className="sr-only" role="status">
        {fetching || loading
          ? "일감을 확인하고 있어요."
          : `${visible.length}개 · ${filter === "today" ? "오늘 마감인 일" : filter === "overdue" ? "마감 시각이 지난 일" : "완료하지 않은 모든 일"}`}
      </p>
      {error && (
        <p className="inline-alert" role="alert">
          {error}{" "}
          {!fetching && (
            <button
              className="text-button focus-visible-control"
              type="button"
              onClick={() => void reload()}
            >
              다시 불러오기
            </button>
          )}
        </p>
      )}
      {(filter !== "today" || workflowFilter !== "all") && (
        <p className="home-direct-tasks__active-scope">
          {text.taskFilterLabel(
            [
              filter === "today"
                ? "오늘"
                : filter === "overdue"
                  ? "기한 지남"
                  : "전체",
              workflowFilter === "not_started"
                ? "진행 전"
                : workflowFilter === "in_progress"
                  ? "진행 중"
                  : workflowFilter === "waiting"
                    ? "대기 중"
                    : "",
            ]
              .filter(Boolean)
              .join(" · "),
          )}
        </p>
      )}
      <div className="home-direct-tasks__groups" ref={rows}>
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
              {items.map((task) => {
                const shared = taskAssignees(task.assigneeName).length > 1;
                return (
                  <li key={task.id}>
                    <div className="mobile-task-row">
                      <button
                        type="button"
                        className="mobile-task-row__complete focus-visible-control"
                        aria-label={`${task.title} 완료하기`}
                        disabled={Boolean(pending)}
                        onClick={(event) =>
                          void complete(task, event.currentTarget)
                        }
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
                        aria-expanded={expanded === `${label}:${task.id}`}
                        aria-controls={`mobile-task-${encodeURIComponent(label)}-${task.id}`}
                        onClick={() =>
                          setExpanded(
                            expanded === `${label}:${task.id}`
                              ? undefined
                              : `${label}:${task.id}`,
                          )
                        }
                      >
                        <strong>{task.title}</strong>
                        <span>
                          {workflowLabel(task)} ·{" "}
                          {grouping === "date"
                            ? task.assigneeName || "담당자 미정"
                            : dueLabel(task.dueAt)}
                          {task.parentTaskId ? " · 하위 일감" : ""}
                          {shared && (
                            <span className="home-direct-tasks__shared">
                              {text.sharedTask}
                            </span>
                          )}
                        </span>
                        <ChevronDown aria-hidden="true" />
                      </button>
                    </div>
                    {expanded === `${label}:${task.id}` && (
                      <div
                        className="mobile-task-detail"
                        id={`mobile-task-${encodeURIComponent(label)}-${task.id}`}
                      >
                        <p>
                          {workflowLabel(task)} ·{" "}
                          {task.assigneeName || "담당자 미정"} ·{" "}
                          {dueLabel(task.dueAt)}
                        </p>
                        {shared && <p>{text.sharedTaskHint}</p>}
                        {task.workflowState === "waiting" &&
                          task.waitingReason && (
                            <p className="home-direct-tasks__waiting">
                              대기 사유: {task.waitingReason}
                            </p>
                          )}
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
                            onClick={(event) =>
                              void complete(task, event.currentTarget)
                            }
                          >
                            <Check aria-hidden="true" />
                            {pending === task.id ? "저장 중" : "완료하기"}
                          </button>
                        </div>
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
      {loaded && !fetching && !visible.length && (
        <p className="mobile-task-queue__empty">
          {workflowFilter !== "all"
            ? text.taskFilterEmpty
            : filter === "today"
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

function workflowLabel(task: Task): string {
  if (task.status === "completed") return "완료";
  return task.workflowState === "waiting"
    ? "대기 중"
    : task.workflowState === "in_progress"
      ? "진행 중"
      : "진행 전";
}
