import { invoke, isTauri } from "@tauri-apps/api/core";

import type { PlanningSnapshot, ScheduleEntry, Task } from "./api/planning";

export type NotificationPermissionState = "granted" | "denied" | "unsupported";

export type NotificationPermissionStatus = {
  status: NotificationPermissionState;
  canRequest: boolean;
};

export type ReminderSyncStatus = "idle" | "syncing" | "ready" | "error";

export type LocalReminder = {
  itemType: "task" | "schedule";
  itemId: string;
  destination: "home" | "calendar" | "projects";
  title: string;
  body?: string;
  projectId?: string;
  targetAtEpochMillis: number;
  triggerAtEpochMillis: number;
};

export type ReminderNavigation = Pick<
  LocalReminder,
  "itemType" | "itemId" | "destination" | "projectId"
> & { targetAtEpochMillis?: number };

type NativeInvoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

type NotificationRuntime = {
  tauri: boolean;
  userAgent: string;
  invoke: NativeInvoke;
};

const reminderLeadTimeMillis = 15 * 60 * 1_000;

export function localNotificationsSupported(): boolean {
  const runtime = currentRuntime();
  return notificationRuntimeAvailable(runtime.tauri, runtime.userAgent);
}

export function notificationRuntimeAvailable(
  tauriRuntime: boolean,
  userAgent: string,
): boolean {
  return tauriRuntime && /Android/i.test(userAgent);
}

export function reminderKey(
  itemType: LocalReminder["itemType"],
  itemId: string,
): string {
  return `${itemType}:${itemId}`;
}

export function reminderFallbackDestination(
  navigation: ReminderNavigation,
): "home" | "calendar" {
  return navigation.destination === "projects"
    ? "calendar"
    : navigation.destination;
}

export function taskReminder(
  task: Task,
  now = Date.now(),
): LocalReminder | undefined {
  if (task.status !== "open" || !task.dueAt) return undefined;
  const dueAt = Date.parse(task.dueAt);
  if (!Number.isFinite(dueAt) || dueAt <= now + 1_000) {
    return undefined;
  }
  return {
    itemType: "task",
    itemId: task.id,
    destination: task.projectId ? "projects" : "calendar",
    title: `곧 마감해요 · ${task.title}`,
    body: "할 일 기한이 다가왔어요. 지금 진행 상황을 확인해 보세요.",
    projectId: task.projectId ?? undefined,
    targetAtEpochMillis: dueAt,
    triggerAtEpochMillis: Math.max(now + 1_000, dueAt - reminderLeadTimeMillis),
  };
}

export function scheduleReminder(
  entry: ScheduleEntry,
  now = Date.now(),
): LocalReminder | undefined {
  if (entry.status !== "confirmed") return undefined;
  const startsAt = Date.parse(entry.startsAt);
  if (!Number.isFinite(startsAt) || startsAt <= now + 1_000) return undefined;
  return {
    itemType: "schedule",
    itemId: entry.id,
    destination: "calendar",
    title: `곧 시작해요 · ${entry.title}`,
    body: "일정 내용을 확인하고 준비해 주세요.",
    targetAtEpochMillis: startsAt,
    triggerAtEpochMillis: Math.max(
      now + 1_000,
      startsAt - reminderLeadTimeMillis,
    ),
  };
}

export async function getNotificationPermissionStatus(
  runtime = currentRuntime(),
): Promise<NotificationPermissionStatus> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return { status: "unsupported", canRequest: false };
  }
  const result = await runtime.invoke<unknown>(
    "plugin:local-notifications|permissionStatus",
  );
  return parsePermissionStatus(result);
}

export async function requestNotificationPermission(
  runtime = currentRuntime(),
): Promise<NotificationPermissionStatus> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return { status: "unsupported", canRequest: false };
  }
  const result = await runtime.invoke<unknown>(
    "plugin:local-notifications|requestPermission",
  );
  return parsePermissionStatus(result);
}

export async function openNotificationSettings(
  runtime = currentRuntime(),
): Promise<boolean> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return false;
  }
  await runtime.invoke("plugin:local-notifications|openSettings");
  return true;
}

export async function scheduleLocalReminder(
  reminder: LocalReminder,
  runtime = currentRuntime(),
): Promise<boolean> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return false;
  }
  validateReminder(reminder);
  await runtime.invoke("plugin:local-notifications|schedule", reminder);
  return true;
}

export async function cancelLocalReminder(
  itemType: LocalReminder["itemType"],
  itemId: string,
  runtime = currentRuntime(),
): Promise<boolean> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return false;
  }
  await runtime.invoke("plugin:local-notifications|cancel", {
    itemType,
    itemId,
  });
  return true;
}

export async function reconcilePlanningReminders(
  snapshot: PlanningSnapshot,
  runtime = currentRuntime(),
  now = Date.now(),
): Promise<void> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) return;
  const operations: Promise<unknown>[] = [];
  const activeKeys: string[] = [];
  for (const task of [...snapshot.tasks, ...snapshot.completedTasks]) {
    const reminder = taskReminder(task, now);
    if (reminder) activeKeys.push(reminderKey("task", task.id));
    operations.push(
      reminder
        ? scheduleLocalReminder(reminder, runtime)
        : cancelLocalReminder("task", task.id, runtime),
    );
  }
  for (const entry of snapshot.schedule) {
    const reminder = scheduleReminder(entry, now);
    if (reminder) activeKeys.push(reminderKey("schedule", entry.id));
    operations.push(
      reminder
        ? scheduleLocalReminder(reminder, runtime)
        : cancelLocalReminder("schedule", entry.id, runtime),
    );
  }
  await Promise.all(operations);
  await reconcileScheduledReminderIndex(activeKeys, runtime);
}

export async function reconcileScheduledReminderIndex(
  activeKeys: string[],
  runtime = currentRuntime(),
): Promise<boolean> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return false;
  }
  await runtime.invoke("plugin:local-notifications|reconcileScheduled", {
    activeKeys,
  });
  return true;
}

export async function takePendingReminderNavigation(
  runtime = currentRuntime(),
): Promise<ReminderNavigation | undefined> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return undefined;
  }
  const result = await runtime.invoke<unknown>(
    "plugin:local-notifications|takePendingNavigation",
  );
  return parseReminderNavigation(result);
}

export async function peekPendingReminderNavigation(
  runtime = currentRuntime(),
): Promise<ReminderNavigation | undefined> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return undefined;
  }
  const result = await runtime.invoke<unknown>(
    "plugin:local-notifications|peekPendingNavigation",
  );
  return parseReminderNavigation(result);
}

export async function acknowledgePendingReminderNavigation(
  navigation: ReminderNavigation,
  runtime = currentRuntime(),
): Promise<boolean> {
  if (!notificationRuntimeAvailable(runtime.tauri, runtime.userAgent)) {
    return false;
  }
  const result = await runtime.invoke<unknown>(
    "plugin:local-notifications|ackPendingNavigation",
    { itemType: navigation.itemType, itemId: navigation.itemId },
  );
  if (!isRecord(result) || typeof result.acknowledged !== "boolean") {
    throw new Error("invalid reminder navigation acknowledgement");
  }
  return result.acknowledged;
}

function parseReminderNavigation(
  result: unknown,
): ReminderNavigation | undefined {
  if (result == null) return undefined;
  if (!isRecord(result)) throw new Error("invalid reminder navigation");
  if (
    (result.itemType !== "task" && result.itemType !== "schedule") ||
    typeof result.itemId !== "string" ||
    (result.destination !== "home" &&
      result.destination !== "calendar" &&
      result.destination !== "projects") ||
    (result.projectId !== undefined && typeof result.projectId !== "string") ||
    (result.targetAtEpochMillis !== undefined &&
      (typeof result.targetAtEpochMillis !== "number" ||
        !Number.isFinite(result.targetAtEpochMillis))) ||
    (result.destination === "projects" && typeof result.projectId !== "string")
  ) {
    throw new Error("invalid reminder navigation");
  }
  return result as ReminderNavigation;
}

function currentRuntime(): NotificationRuntime {
  return {
    tauri: isTauri(),
    userAgent: globalThis.navigator?.userAgent ?? "",
    invoke,
  };
}

function parsePermissionStatus(value: unknown): NotificationPermissionStatus {
  if (
    !isRecord(value) ||
    (value.status !== "granted" && value.status !== "denied") ||
    typeof value.canRequest !== "boolean"
  ) {
    throw new Error("invalid notification permission status");
  }
  return { status: value.status, canRequest: value.canRequest };
}

function validateReminder(reminder: LocalReminder) {
  if (
    !reminder.itemId ||
    !reminder.title.trim() ||
    reminder.title.length > 120 ||
    (reminder.body?.length ?? 0) > 240 ||
    !Number.isSafeInteger(reminder.triggerAtEpochMillis) ||
    reminder.triggerAtEpochMillis <= Date.now()
  ) {
    throw new Error("invalid local reminder");
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
