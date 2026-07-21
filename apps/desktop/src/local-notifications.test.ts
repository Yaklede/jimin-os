import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ScheduleEntry, Task } from "./api/planning";
import {
  acknowledgePendingReminderNavigation,
  cancelLocalReminder,
  getNotificationPermissionStatus,
  getNativePushToken,
  notificationRuntimeAvailable,
  openNotificationSettings,
  peekPendingReminderNavigation,
  reconcilePlanningReminders,
  reminderFallbackDestination,
  reminderKey,
  requestNotificationPermission,
  scheduleLocalReminder,
  scheduleReminder,
  taskReminder,
  takePendingReminderNavigation,
} from "./local-notifications";

const now = Date.parse("2026-07-15T00:00:00.000Z");

const task: Task = {
  id: "019f68cb-9400-7000-8000-000000000001",
  projectId: null,
  title: "계약서 검토",
  notes: null,
  status: "open",
  priority: 2,
  dueAt: "2026-07-15T09:00:00.000Z",
  completedAt: null,
  version: 1,
};

const schedule: ScheduleEntry = {
  id: "019f68cb-9400-7000-8000-000000000002",
  title: "주간 회의",
  notes: null,
  startsAt: "2026-07-15T10:00:00.000Z",
  endsAt: "2026-07-15T11:00:00.000Z",
  timeZone: "Asia/Seoul",
  status: "confirmed",
  source: "manual",
  editable: true,
  version: 1,
};

function androidRuntime(nativeInvoke = vi.fn().mockResolvedValue(undefined)) {
  return {
    tauri: true,
    userAgent: "Mozilla/5.0 (Linux; Android 16)",
    invoke: nativeInvoke,
  };
}

describe("local notification runtime", () => {
  it("only enables native reminders for Android Tauri", () => {
    expect(notificationRuntimeAvailable(true, "Android 16")).toBe(true);
    expect(notificationRuntimeAvailable(false, "Android 16")).toBe(false);
    expect(notificationRuntimeAvailable(true, "Macintosh")).toBe(false);
  });

  it("uses a stable type-qualified reminder key", () => {
    expect(reminderKey("task", "same-id")).toBe("task:same-id");
    expect(reminderKey("schedule", "same-id")).toBe("schedule:same-id");
  });

  it("falls back to the calendar when a project task cannot be resolved", () => {
    expect(
      reminderFallbackDestination({
        itemType: "task",
        itemId: task.id,
        destination: "projects",
        projectId: "missing-project",
      }),
    ).toBe("calendar");
  });

  it("builds task and schedule reminders fifteen minutes early", () => {
    expect(taskReminder(task, now)).toMatchObject({
      itemType: "task",
      destination: "calendar",
      targetAtEpochMillis: Date.parse(task.dueAt!),
      triggerAtEpochMillis: Date.parse(task.dueAt!) - 15 * 60 * 1_000,
    });
    expect(scheduleReminder(schedule, now)).toMatchObject({
      itemType: "schedule",
      destination: "calendar",
      targetAtEpochMillis: Date.parse(schedule.startsAt),
      triggerAtEpochMillis: Date.parse(schedule.startsAt) - 15 * 60 * 1_000,
    });
  });

  it("keeps the project identity for project task navigation", () => {
    const projectTask = {
      ...task,
      projectId: "019f68cb-9400-7000-8000-000000000099",
    };
    expect(taskReminder(projectTask, now)).toMatchObject({
      destination: "projects",
      projectId: projectTask.projectId,
    });
  });

  it("does not schedule completed, cancelled, or expired items", () => {
    expect(taskReminder({ ...task, status: "completed" }, now)).toBeUndefined();
    expect(
      scheduleReminder({ ...schedule, status: "cancelled" }, now),
    ).toBeUndefined();
    expect(
      scheduleReminder({ ...schedule, startsAt: "2026-07-14T23:00:00Z" }, now),
    ).toBeUndefined();
  });
});

describe("local notification bridge", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(now);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reads and requests Android permission in user context", async () => {
    const nativeInvoke = vi
      .fn()
      .mockResolvedValueOnce({ status: "denied", canRequest: true })
      .mockResolvedValueOnce({ status: "granted", canRequest: false });
    const runtime = androidRuntime(nativeInvoke);

    await expect(getNotificationPermissionStatus(runtime)).resolves.toEqual({
      status: "denied",
      canRequest: true,
    });
    await expect(requestNotificationPermission(runtime)).resolves.toEqual({
      status: "granted",
      canRequest: false,
    });
    expect(nativeInvoke.mock.calls.map(([command]) => command)).toEqual([
      "plugin:local-notifications|permissionStatus",
      "plugin:local-notifications|requestPermission",
    ]);
  });

  it("reads a bounded FCM registration handle from the Android bridge", async () => {
    const nativeInvoke = vi.fn().mockResolvedValue({
      state: "ready",
      registrationHandle: "fcm-registration-handle-for-jimin-os",
    });
    await expect(
      getNativePushToken(androidRuntime(nativeInvoke)),
    ).resolves.toEqual({
      state: "ready",
      registrationHandle: "fcm-registration-handle-for-jimin-os",
    });
    expect(nativeInvoke).toHaveBeenCalledWith(
      "plugin:local-notifications|pushToken",
    );
  });

  it("does not expose a registration handle when Firebase is not configured", async () => {
    const nativeInvoke = vi.fn().mockResolvedValue({ state: "unconfigured" });
    await expect(
      getNativePushToken(androidRuntime(nativeInvoke)),
    ).resolves.toEqual({ state: "unconfigured" });
  });

  it("schedules and cancels using the same item identity", async () => {
    const nativeInvoke = vi.fn().mockResolvedValue(undefined);
    const runtime = androidRuntime(nativeInvoke);
    const reminder = taskReminder(task, now)!;

    await expect(scheduleLocalReminder(reminder, runtime)).resolves.toBe(true);
    await expect(cancelLocalReminder("task", task.id, runtime)).resolves.toBe(
      true,
    );
    expect(nativeInvoke).toHaveBeenNthCalledWith(
      1,
      "plugin:local-notifications|schedule",
      reminder,
    );
    expect(nativeInvoke).toHaveBeenNthCalledWith(
      2,
      "plugin:local-notifications|cancel",
      { itemType: "task", itemId: task.id },
    );
  });

  it("opens Android application notification settings for denied recovery", async () => {
    const nativeInvoke = vi.fn().mockResolvedValue(undefined);
    await expect(
      openNotificationSettings(androidRuntime(nativeInvoke)),
    ).resolves.toBe(true);
    expect(nativeInvoke).toHaveBeenCalledWith(
      "plugin:local-notifications|openSettings",
    );
  });

  it("reconciles active reminders and cancels terminal items", async () => {
    const nativeInvoke = vi.fn().mockResolvedValue(undefined);
    const runtime = androidRuntime(nativeInvoke);
    await reconcilePlanningReminders(
      {
        tasks: [task],
        completedTasks: [{ ...task, id: "completed", status: "completed" }],
        schedule: [
          schedule,
          { ...schedule, id: "cancelled", status: "cancelled" },
        ],
      },
      runtime,
      now,
    );

    expect(nativeInvoke.mock.calls.map(([command]) => command)).toEqual([
      "plugin:local-notifications|schedule",
      "plugin:local-notifications|cancel",
      "plugin:local-notifications|schedule",
      "plugin:local-notifications|cancel",
      "plugin:local-notifications|reconcileScheduled",
    ]);
    expect(nativeInvoke).toHaveBeenLastCalledWith(
      "plugin:local-notifications|reconcileScheduled",
      {
        activeKeys: [
          reminderKey("task", task.id),
          reminderKey("schedule", schedule.id),
        ],
      },
    );
  });

  it("returns the screen and item selected from a notification", async () => {
    const runtime = androidRuntime(
      vi.fn().mockResolvedValue({
        itemType: "schedule",
        itemId: schedule.id,
        destination: "calendar",
      }),
    );
    await expect(takePendingReminderNavigation(runtime)).resolves.toEqual({
      itemType: "schedule",
      itemId: schedule.id,
      destination: "calendar",
    });
  });

  it("keeps pending navigation until the destination acknowledges it", async () => {
    const navigation = {
      itemType: "task" as const,
      itemId: task.id,
      destination: "projects" as const,
      projectId: "019f68cb-9400-7000-8000-000000000099",
    };
    const nativeInvoke = vi
      .fn()
      .mockResolvedValueOnce(navigation)
      .mockResolvedValueOnce({ acknowledged: true });
    const runtime = androidRuntime(nativeInvoke);

    await expect(peekPendingReminderNavigation(runtime)).resolves.toEqual(
      navigation,
    );
    await expect(
      acknowledgePendingReminderNavigation(navigation, runtime),
    ).resolves.toBe(true);
    expect(nativeInvoke).toHaveBeenNthCalledWith(
      1,
      "plugin:local-notifications|peekPendingNavigation",
    );
    expect(nativeInvoke).toHaveBeenNthCalledWith(
      2,
      "plugin:local-notifications|ackPendingNavigation",
      { itemType: navigation.itemType, itemId: navigation.itemId },
    );
  });

  it("rejects a pending navigation with an unsupported semantic destination", async () => {
    const runtime = androidRuntime(
      vi.fn().mockResolvedValue({
        itemType: "schedule",
        itemId: schedule.id,
        destination: "settings",
        targetAtEpochMillis: Date.parse(schedule.startsAt),
      }),
    );
    await expect(peekPendingReminderNavigation(runtime)).rejects.toThrow(
      "invalid reminder navigation",
    );
  });

  it("is a no-op outside Android instead of requesting a desktop permission", async () => {
    const nativeInvoke = vi.fn();
    const runtime = { ...androidRuntime(nativeInvoke), userAgent: "Macintosh" };
    await expect(getNotificationPermissionStatus(runtime)).resolves.toEqual({
      status: "unsupported",
      canRequest: false,
    });
    expect(nativeInvoke).not.toHaveBeenCalled();
  });
});
