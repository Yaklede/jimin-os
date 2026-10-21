import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { PlanningRequestError } from "./api/planning";
import { openDeckCopy } from "./open-deck-copy";

export interface DeckAction {
  operation: "navigate" | "refresh" | "open-schedule" | "open-task";
  view?: string;
  itemId?: string;
}
export interface DeckSnapshot {
  schemaVersion: 1;
  updatedAt: number;
  connected: boolean;
  timeZone: string;
  range: { from: string; to: string };
  counts: {
    requests: number;
    updates: number;
    uncertain: number;
    today: number;
    overdue: number;
  };
  events: {
    id: string;
    title: string;
    startsAt: string;
    endsAt: string;
    source: string;
  }[];
  tasks: {
    id: string;
    title: string;
    dueAt: string | null;
    assignee: string;
    state: string;
  }[];
  inbox: {
    id: string;
    title: string;
    projectName: string;
    receivedAt: string;
    kind: string;
    revision: number;
  }[];
  routines: {
    id: string;
    title: string;
    nextRunAt: string;
    enabled: boolean;
  }[];
}
type Row = Record<string, unknown>;
const record = (v: unknown): v is Row =>
  !!v && typeof v === "object" && !Array.isArray(v);
const rows = (v: unknown) => (Array.isArray(v) ? v.filter(record) : []);
const text = (v: unknown, max = 160) =>
  typeof v === "string" ? v.slice(0, max) : "";
const validId = (v: unknown) =>
  typeof v === "string" && /^[0-9a-f-]{36}$/i.test(v);
const date = (v: unknown) =>
  typeof v === "string" && Number.isFinite(Date.parse(v)) ? v : "";
const koreanDay = (v: string) =>
  new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Seoul",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(v));
export function deckRange(now: Date) {
  const day = koreanDay(now.toISOString()),
    [year, month] = day.split("-").map(Number);
  return {
    from: new Date(Date.UTC(year, month - 1, 1, -9)).toISOString(),
    to: new Date(Date.UTC(year, month + 1, 1, -9)).toISOString(),
  };
}
export function normalizeDeckSnapshot(
  home: Row,
  schedule: unknown,
  routines: unknown,
  now = new Date(),
): DeckSnapshot {
  const pending = rows(home.inflow).filter(
      (x) => x.status === "pending" && !x.reviewed,
    ),
    tasks = [
      ...new Map(
        [...rows(home.tasks), ...rows(home.dueTasks)]
          .filter((x) => x.status === "open")
          .map((x) => [x.id, x]),
      ).values(),
    ];
  const uncertain = (x: Row) =>
    !x.promotedTaskId &&
    (x.analysisStatus === "failed" ||
      (x.analysisStatus === "ready" &&
        x.analysisClassification !== "new_task"));
  const today = koreanDay(now.toISOString());
  const range = deckRange(now);
  return {
    schemaVersion: 1,
    updatedAt: now.getTime(),
    connected: true,
    timeZone: "Asia/Seoul",
    range: deckRange(now),
    counts: {
      requests: pending.filter((x) => !x.promotedTaskId && !uncertain(x))
        .length,
      updates: pending.filter((x) => !!x.promotedTaskId).length,
      uncertain: pending.filter(uncertain).length,
      today: tasks.filter(
        (x) => date(x.dueAt) && koreanDay(String(x.dueAt)) === today,
      ).length,
      overdue: tasks.filter(
        (x) => date(x.dueAt) && Date.parse(String(x.dueAt)) < now.getTime(),
      ).length,
    },
    events: rows(schedule)
      .filter(
        (x) =>
          validId(x.id) &&
          x.status !== "cancelled" &&
          date(x.startsAt) &&
          date(x.endsAt) &&
          Date.parse(String(x.endsAt)) > Date.parse(range.from) &&
          Date.parse(String(x.startsAt)) < Date.parse(range.to),
      )
      .slice(0, 120)
      .map((x) => ({
        id: String(x.id),
        title: text(x.title),
        startsAt: String(x.startsAt),
        endsAt: String(x.endsAt),
        source: text(x.source, 30),
      }))
      .sort((a, b) => a.startsAt.localeCompare(b.startsAt)),
    tasks: tasks
      .filter((x) => validId(x.id))
      .sort((a, b) =>
        (date(a.dueAt) || "9999").localeCompare(date(b.dueAt) || "9999"),
      )
      .slice(0, 40)
      .map((x) => ({
        id: String(x.id),
        title: text(x.title),
        dueAt: date(x.dueAt) || null,
        assignee: text(x.assigneeName, 60),
        state: text(x.workflowState || "not_started", 30),
      })),
    inbox: pending
      .filter((x) => validId(x.id))
      .slice(0, 30)
      .map((x) => ({
        id: String(x.id),
        title: text(
          x.suggestedTaskTitle ||
            x.analysisSummary ||
            openDeckCopy.requestTitle,
        ),
        projectName: text(x.projectName, 60),
        receivedAt: date(x.receivedAt),
        kind: x.promotedTaskId
          ? "update"
          : uncertain(x)
            ? "uncertain"
            : "request",
        revision: Number(x.sourceRevision || x.version || 0),
      })),
    routines: rows(routines)
      .filter((x) => validId(x.id))
      .slice(0, 20)
      .map((x) => ({
        id: String(x.id),
        title: text(
          record(x.definition)
            ? x.definition.title
            : openDeckCopy.scheduledWorkTitle,
        ),
        nextRunAt: date(x.nextRunAt),
        enabled: x.enabled === true,
      })),
  };
}
export async function loadDeckSnapshot(
  base: string,
  access: string,
  now = new Date(),
  fetcher = fetch,
): Promise<DeckSnapshot> {
  const range = deckRange(now),
    query = new URLSearchParams(range);
  const scheduleQuery = new URLSearchParams({
    from: new Date(Date.parse(range.from) - 86400000).toISOString(),
    to: new Date(Date.parse(range.to) + 86400000).toISOString(),
  });
  const get = async (path: string): Promise<Row> => {
    const res = await fetcher(base.replace(/\/$/, "") + path, {
      headers: {
        Authorization: "Bearer " + access,
        Accept: "application/json",
      },
      signal: AbortSignal.timeout(6000),
    });
    if (!res.ok)
      throw new PlanningRequestError(
        res.status === 401 ? "unauthorized" : "unavailable",
      );
    const body: unknown = await res.json();
    if (!record(body)) throw new PlanningRequestError("unavailable");
    return body;
  };
  const [home, schedule, routines] = await Promise.all([
    get("/v1/home?" + query),
    get("/v1/schedule-entries?" + scheduleQuery),
    get("/v1/scheduled-work"),
  ]);
  return normalizeDeckSnapshot(home, schedule.items, routines.items, now);
}
export class DeckActionDispatcher {
  private workers = new Map<
    string,
    { controller: AbortController; timer: ReturnType<typeof setTimeout> }
  >();
  private disposed = false;
  constructor(
    private act: (action: DeckAction, signal: AbortSignal) => Promise<void>,
    private finish: (id: string, error?: string) => void,
    private timeout = 6500,
  ) {}
  run(id: string, action: DeckAction) {
    if (this.disposed || this.workers.has(id)) return;
    if (this.workers.size >= 4) {
      this.finish(id, "BUSY");
      return;
    }
    const controller = new AbortController();
    let finished = false;
    const done = (error?: string) => {
      if (!finished && !this.disposed) {
        finished = true;
        this.finish(id, error);
      }
    };
    const timer = setTimeout(() => {
      controller.abort();
      done("ACTION_TIMEOUT");
    }, this.timeout);
    this.workers.set(id, { controller, timer });
    void Promise.resolve()
      .then(() => this.act(action, controller.signal))
      .then(
        () => done(),
        (failure) => {
          const code = failure instanceof Error ? failure.message : "";
          done(
            ["BUSY_EDITING", "NOT_FOUND", "NOT_CONNECTED"].includes(code)
              ? code
              : "ACTION_FAILED",
          );
        },
      )
      .finally(() => {
        clearTimeout(timer);
        this.workers.delete(id);
      });
  }
  dispose() {
    this.disposed = true;
    for (const worker of this.workers.values()) {
      clearTimeout(worker.timer);
      worker.controller.abort();
    }
  }
}
interface Options {
  enabled: boolean;
  load(): Promise<DeckSnapshot>;
  act(action: DeckAction, signal: AbortSignal): Promise<void>;
}
export function useOpenDeckBridge(options: Options) {
  const latest = useRef(options);
  latest.current = options;
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window) || !options.enabled) return;
    // This command exists only in the macOS build. Mobile clients keep their own reminder flow.
    let disposed = false,
      busy = false;
    let stop: undefined | (() => void);
    const publish = async () => {
      if (busy || disposed) return;
      busy = true;
      try {
        const snapshot = await latest.current.load();
        if (!disposed) await invoke("publish_open_deck_snapshot", { snapshot });
      } catch {
        if (!disposed)
          await invoke("publish_open_deck_snapshot", {
            snapshot: {
              schemaVersion: 1,
              updatedAt: Date.now(),
              connected: false,
              timeZone: "Asia/Seoul",
              events: [],
              tasks: [],
              inbox: [],
              routines: [],
              range: deckRange(new Date()),
              counts: {
                requests: 0,
                updates: 0,
                uncertain: 0,
                today: 0,
                overdue: 0,
              },
            },
          }).catch(() => {});
      } finally {
        busy = false;
      }
    };
    const dispatcher = new DeckActionDispatcher(
      async (action, signal) => {
        if (action.operation === "refresh") await publish();
        else await latest.current.act(action, signal);
      },
      (requestId, errorCode) => {
        void invoke("finish_open_deck_action", {
          requestId,
          errorCode: errorCode ?? null,
        }).catch(() => {});
      },
    );
    void listen<{ requestId: string; action: DeckAction }>(
      "open-deck-action",
      ({ payload }) => dispatcher.run(payload.requestId, payload.action),
    )
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch(() => {});
    void publish();
    const timer = window.setInterval(() => void publish(), 15000);
    return () => {
      disposed = true;
      dispatcher.dispose();
      window.clearInterval(timer);
      stop?.();
    };
  }, [options.enabled]);
}
