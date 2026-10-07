import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  configureDesktopInflowNotifications,
  desktopInflowNotificationsSupported,
  desktopInflowNotificationsEnabled,
  setDesktopInflowNotificationsEnabled,
  desktopNotificationPreferenceEvent,
} from "./desktop-inflow-notifications";

const native = vi.hoisted(() => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => native);

describe("desktop inflow notifications", () => {
  let stored = new Map<string, string>();
  beforeEach(() => {
    stored = new Map();
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X)",
    });
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => stored.get(key) ?? null,
      setItem: (key: string, value: string) => stored.set(key, value),
    });
    vi.stubGlobal("window", { dispatchEvent: vi.fn() });
    vi.stubGlobal(
      "CustomEvent",
      class {
        constructor(
          public type: string,
          public options: unknown,
        ) {}
      },
    );
    native.isTauri.mockReturnValue(true);
    native.invoke.mockReset();
  });
  it("never runs the desktop poller in a browser or Android", async () => {
    native.isTauri.mockReturnValue(false);
    expect(desktopInflowNotificationsSupported()).toBe(false);
    await configureDesktopInflowNotifications(
      "https://example.test",
      "fixture-access",
    );
    expect(native.invoke).not.toHaveBeenCalled();
    native.isTauri.mockReturnValue(true);
    vi.stubGlobal("navigator", { userAgent: "Android" });
    expect(desktopInflowNotificationsSupported()).toBe(false);
  });
  it("starts a native worker and explicitly clears credentials at logout", async () => {
    await configureDesktopInflowNotifications(
      "https://example.test",
      "fixture-access",
    );
    expect(native.invoke).toHaveBeenCalledWith(
      "configure_inflow_notifications",
      { baseUrl: "https://example.test", access: "fixture-access" },
    );
    await configureDesktopInflowNotifications("https://example.test");
    expect(native.invoke).toHaveBeenLastCalledWith(
      "configure_inflow_notifications",
      { baseUrl: "https://example.test", access: null },
    );
  });
  it("persists opt-out and immediately broadcasts it", () => {
    expect(desktopInflowNotificationsEnabled()).toBe(true);
    setDesktopInflowNotificationsEnabled(false);
    expect(desktopInflowNotificationsEnabled()).toBe(false);
    expect(window.dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({ type: desktopNotificationPreferenceEvent }),
    );
  });
});
