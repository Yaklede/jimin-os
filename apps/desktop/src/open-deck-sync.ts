import type { DeckSnapshot } from "./open-deck-bridge";

export interface DeckDiagnostic {
  route: string;
  code: string;
  httpStatus: number;
  durationMs: number;
}
export interface DeckSync {
  state: "fresh" | "retrying";
  attemptedAt: number;
  retryAt: number;
  failures: number;
  error?: DeckDiagnostic;
}
export function safeDeckDiagnostic(cause: unknown): DeckDiagnostic {
  const value = (cause as { diagnostic?: Partial<DeckDiagnostic> })?.diagnostic;
  return {
    route: ["/v1/home", "/v1/schedule-entries", "/v1/scheduled-work"].includes(
      value?.route ?? "",
    )
      ? value!.route!
      : "snapshot",
    code: ["http", "timeout", "network"].includes(value?.code ?? "")
      ? value!.code!
      : "unavailable",
    httpStatus: Number.isInteger(value?.httpStatus)
      ? Math.max(0, Math.min(599, value!.httpStatus!))
      : 0,
    durationMs: Number.isFinite(value?.durationMs)
      ? Math.max(0, Math.min(120000, value!.durationMs!))
      : 0,
  };
}
export class DeckSnapshotPublisher {
  private last: DeckSnapshot | undefined;
  private failures = 0;
  private disposed = false;
  private pending: Promise<number> | undefined;
  constructor(
    private load: () => Promise<DeckSnapshot>,
    private publish: (snapshot: DeckSnapshot) => Promise<unknown>,
    private log: (entry: Record<string, unknown>) => Promise<unknown>,
    private clock = () => Date.now(),
  ) {}
  refresh(): Promise<number> {
    if (this.disposed) return Promise.resolve(15000);
    // Focus, timer and mutation refreshes share the same read and completion.
    if (!this.pending)
      this.pending = this.run().finally(() => {
        this.pending = undefined;
      });
    return this.pending;
  }
  private async run(): Promise<number> {
    const attemptedAt = this.clock();
    let snapshot: DeckSnapshot;
    let delay = 15000;
    try {
      snapshot = await this.load();
      snapshot = {
        ...snapshot,
        sync: { state: "fresh", attemptedAt, retryAt: 0, failures: 0 },
      };
      this.last = snapshot;
      if (this.failures && !this.disposed)
        await this.record({
          event: "recovered",
          at: this.clock(),
          failures: this.failures,
        });
      this.failures = 0;
    } catch (cause) {
      this.failures++;
      delay = [2000, 5000, 10000, 30000][Math.min(this.failures - 1, 3)];
      const error = safeDeckDiagnostic(cause);
      const sync: DeckSync = {
        state: "retrying",
        attemptedAt,
        retryAt: this.clock() + delay,
        failures: this.failures,
        error,
      };
      snapshot = {
        ...(this.last ?? {
          schemaVersion: 1,
          updatedAt: 0,
          timeZone: "Asia/Seoul",
          range: { from: "", to: "" },
          counts: {
            requests: 0,
            updates: 0,
            uncertain: 0,
            today: 0,
            overdue: 0,
          },
          events: [],
          tasks: [],
          inbox: [],
          routines: [],
        }),
        connected: false,
        capabilities: { scheduleCreate: false, taskComplete: false },
        sync,
      };
      // No tokens, queries, response bodies, titles or arbitrary error messages.
      if (!this.disposed)
        await this.record({
          event: "refresh_failed",
          at: this.clock(),
          failures: this.failures,
          retryAt: sync.retryAt,
          ...error,
        });
    }
    if (!this.disposed) {
      try {
        await this.publish(snapshot);
      } catch {
        await this.record({
          event: "publish_failed",
          at: this.clock(),
          failures: this.failures,
        }).catch(() => {});
      }
    }
    return delay;
  }
  private async record(entry: Record<string, unknown>) {
    await this.log(entry).catch(() => {});
  }
  dispose() {
    this.disposed = true;
  }
}
