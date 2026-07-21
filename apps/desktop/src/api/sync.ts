import { PlanningRequestError } from "./planning";

export interface SyncChange {
  sequence: string;
  entityType: string;
  entityId: string;
  operation: "upsert" | "delete";
  entityVersion: number;
  changedAt: string;
}

export interface SyncChangePage {
  items: SyncChange[];
  nextCursor: string;
  currentCursor: string;
  hasMore: boolean;
}

export async function fetchSyncChanges(
  baseUrl: string,
  access: string,
  after: string,
): Promise<SyncChangePage> {
  const url = new URL(`${normalizeBaseUrl(baseUrl)}/v1/sync/changes`);
  url.searchParams.set("after", after);
  url.searchParams.set("limit", "200");
  const response = await fetch(url, {
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${access}`,
    },
  });
  const body: unknown = await response.json().catch(() => undefined);
  if (!response.ok) throw errorFromStatus(response.status);
  if (!isSyncChangePage(body)) throw new PlanningRequestError("unavailable");
  return body;
}

export async function streamSyncCursor(
  baseUrl: string,
  access: string,
  after: string,
  signal: AbortSignal,
  onCursor: (cursor: string) => void,
): Promise<void> {
  const url = new URL(`${normalizeBaseUrl(baseUrl)}/v1/sync/stream`);
  url.searchParams.set("after", after);
  const response = await fetch(url, {
    headers: {
      Accept: "text/event-stream",
      Authorization: `Bearer ${access}`,
    },
    signal,
  });
  if (!response.ok || !response.body) throw errorFromStatus(response.status);

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let pending = "";
  try {
    while (!signal.aborted) {
      const { done, value } = await reader.read();
      if (done) break;
      pending += decoder.decode(value, { stream: true });
      const frames = pending.split("\n\n");
      pending = frames.pop() ?? "";
      for (const frame of frames) {
        const cursor = parseCursorFrame(frame);
        if (cursor) onCursor(cursor);
      }
    }
  } finally {
    reader.releaseLock();
  }
}

export function parseCursorFrame(frame: string): string | undefined {
  const event = frame
    .split("\n")
    .find((line) => line.startsWith("event:"))
    ?.slice("event:".length)
    .trim();
  if (event !== "cursor") return undefined;
  const data = frame
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice("data:".length).trimStart())
    .join("\n");
  if (!data) return undefined;
  try {
    const value: unknown = JSON.parse(data);
    return isCursor((value as { cursor?: unknown })?.cursor)
      ? (value as { cursor: string }).cursor
      : undefined;
  } catch {
    return undefined;
  }
}

function isSyncChangePage(value: unknown): value is SyncChangePage {
  if (!value || typeof value !== "object") return false;
  const page = value as Partial<SyncChangePage>;
  return (
    Array.isArray(page.items) &&
    page.items.every(isSyncChange) &&
    isCursor(page.nextCursor) &&
    isCursor(page.currentCursor) &&
    typeof page.hasMore === "boolean"
  );
}

function isSyncChange(value: unknown): value is SyncChange {
  if (!value || typeof value !== "object") return false;
  const change = value as Partial<SyncChange>;
  return (
    isCursor(change.sequence) &&
    typeof change.entityType === "string" &&
    typeof change.entityId === "string" &&
    ["upsert", "delete"].includes(change.operation ?? "") &&
    typeof change.entityVersion === "number" &&
    typeof change.changedAt === "string"
  );
}

function isCursor(value: unknown): value is string {
  return typeof value === "string" && /^(0|[1-9]\d*)$/.test(value);
}

function errorFromStatus(status: number): PlanningRequestError {
  if (status === 401) return new PlanningRequestError("unauthorized");
  if (status === 400) return new PlanningRequestError("invalid");
  if (status === 409) return new PlanningRequestError("conflict");
  return new PlanningRequestError("unavailable");
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/+$/, "");
}
