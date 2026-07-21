const storageKey = "jimin-os-sync-cursor-v1";

export function readSyncCursor(): string | undefined {
  const value = localStorage.getItem(storageKey);
  return isSyncCursor(value) ? value : undefined;
}

export function writeSyncCursor(value: string): void {
  if (!isSyncCursor(value)) return;
  localStorage.setItem(storageKey, value);
}

export function laterSyncCursor(
  left: string | undefined,
  right: string | undefined,
): string {
  const leftValue = parseSyncCursor(left);
  const rightValue = parseSyncCursor(right);
  return leftValue >= rightValue ? leftValue.toString() : rightValue.toString();
}

export function earlierSyncCursor(
  left: string | undefined,
  right: string | undefined,
): string {
  if (left === undefined) return parseSyncCursor(right).toString();
  if (right === undefined) return parseSyncCursor(left).toString();
  const leftValue = parseSyncCursor(left);
  const rightValue = parseSyncCursor(right);
  return leftValue <= rightValue ? leftValue.toString() : rightValue.toString();
}

function parseSyncCursor(value: string | undefined): bigint {
  return isSyncCursor(value) ? BigInt(value) : 0n;
}

function isSyncCursor(value: unknown): value is string {
  return typeof value === "string" && /^(0|[1-9]\d*)$/.test(value);
}
