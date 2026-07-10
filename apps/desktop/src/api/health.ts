export type CheckStatus = "ok" | "error";

export interface LiveHealthResponse {
  status: "ok";
  service: "api";
  buildSha: string;
}

export interface ReadyHealthResponse {
  status: "ready" | "notReady";
  checks: {
    configuration: CheckStatus;
    database: CheckStatus;
    migrations: CheckStatus;
  };
  schemaVersion: number;
}

export interface HealthSnapshot {
  live: LiveHealthResponse;
  ready: ReadyHealthResponse;
  checkedAt: Date;
}

type FetchLike = typeof fetch;

const REQUEST_TIMEOUT_MS = 5_000;

export class HealthRequestError extends Error {
  constructor() {
    super("server status unavailable");
    this.name = "HealthRequestError";
  }
}

export async function fetchServerHealth(
  baseUrl: string,
  fetchImpl: FetchLike = fetch,
): Promise<HealthSnapshot> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  const base = baseUrl.replace(/\/$/, "");

  try {
    const [liveResponse, readyResponse] = await Promise.all([
      fetchImpl(`${base}/health/live`, {
        headers: { Accept: "application/json" },
        signal: controller.signal,
      }),
      fetchImpl(`${base}/health/ready`, {
        headers: { Accept: "application/json" },
        signal: controller.signal,
      }),
    ]);

    if (!liveResponse.ok || ![200, 503].includes(readyResponse.status)) {
      throw new HealthRequestError();
    }

    const live = parseLive(await liveResponse.json());
    const ready = parseReady(await readyResponse.json());

    return { live, ready, checkedAt: new Date() };
  } catch {
    throw new HealthRequestError();
  } finally {
    clearTimeout(timeout);
  }
}

export function formatCheckedAt(value: Date): string {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(value);
}

function parseLive(value: unknown): LiveHealthResponse {
  if (!isRecord(value)) {
    throw new HealthRequestError();
  }
  if (
    value.status !== "ok" ||
    value.service !== "api" ||
    typeof value.buildSha !== "string"
  ) {
    throw new HealthRequestError();
  }
  return {
    status: value.status,
    service: value.service,
    buildSha: value.buildSha,
  };
}

function parseReady(value: unknown): ReadyHealthResponse {
  if (!isRecord(value) || !isRecord(value.checks)) {
    throw new HealthRequestError();
  }
  if (
    !["ready", "notReady"].includes(String(value.status)) ||
    !isCheckStatus(value.checks.configuration) ||
    !isCheckStatus(value.checks.database) ||
    !isCheckStatus(value.checks.migrations) ||
    typeof value.schemaVersion !== "number" ||
    !Number.isSafeInteger(value.schemaVersion)
  ) {
    throw new HealthRequestError();
  }

  return {
    status: value.status as ReadyHealthResponse["status"],
    checks: {
      configuration: value.checks.configuration,
      database: value.checks.database,
      migrations: value.checks.migrations,
    },
    schemaVersion: value.schemaVersion,
  };
}

function isCheckStatus(value: unknown): value is CheckStatus {
  return value === "ok" || value === "error";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
