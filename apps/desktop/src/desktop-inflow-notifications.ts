import { invoke, isTauri } from "@tauri-apps/api/core";

const preferenceKey = "jimin-os.desktop-inflow-notifications.v1";
export const desktopNotificationPreferenceEvent =
  "desktop-inflow-notification-preference";

export function desktopInflowNotificationsSupported(): boolean {
  return (
    isTauri() &&
    /Macintosh|Mac OS X/i.test(navigator.userAgent) &&
    !/Android|iPhone|iPad/i.test(navigator.userAgent)
  );
}

export function desktopInflowNotificationsEnabled(): boolean {
  try {
    return localStorage.getItem(preferenceKey) !== "off";
  } catch {
    return true;
  }
}

export function setDesktopInflowNotificationsEnabled(enabled: boolean): void {
  try {
    localStorage.setItem(preferenceKey, enabled ? "on" : "off");
  } catch {
    /* The current session still applies the choice. */
  }
  window.dispatchEvent(
    new CustomEvent(desktopNotificationPreferenceEvent, { detail: enabled }),
  );
}

export async function configureDesktopInflowNotifications(
  baseUrl: string,
  access?: string,
): Promise<void> {
  if (!desktopInflowNotificationsSupported()) return;
  await invoke("configure_inflow_notifications", {
    baseUrl,
    access: access ?? null,
  });
}
